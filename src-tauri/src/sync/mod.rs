use std::sync::Arc;
use tokio::sync::Mutex;
use crate::queue::{self, PrintQueue, JobStatus};
use crate::printer::engine::PrintEngine;
use crate::logs::AgentLogger;

pub struct SyncEngine {
    pub api_client: Arc<super::api::ApiClient>,
    pub db: Arc<Mutex<rusqlite::Connection>>,
}

impl SyncEngine {
    pub fn new(api_client: Arc<super::api::ApiClient>, db: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self { api_client, db }
    }

    /// جلب الوظائف المعلقة من السيرفر وإضافتها إلى الطابور المحلي
    /// (idempotent: الوظيفة المحفوظة تُعاد بلا تكرار)
    pub async fn sync_pending_jobs(&self, token: &str) -> Result<u32, String> {
        let jobs = self.api_client.fetch_pending_jobs(token).await?;
        let mut count = 0u32;

        let db = self.db.lock().await;
        for job in &jobs {
            let create = queue::CreateJob {
                job_id: job.id,
                store_id: job.store_id,
                printer_id: job.printer_id,
                section_id: job.print_section_id,
                payload: job.payload.to_string(),
                // عدد المحاولات من تعريف الطابعة في Ordoo Agent (افتراضي3)
                max_attempts: job.max_attempts.unwrap_or(3),
            };
            if PrintQueue::enqueue(&db, &create).is_ok() {
                count += 1;
            }
        }

        Ok(count)
    }

    /// مزامنة طابعات السيرفر إلى الجدول المحلي (ربط دائم بـ server_printer_id)
    /// بدونها لا تجد الوظيفة طابعتها عند التنفيذ
    pub async fn sync_printers(&self, token: &str) -> Result<u32, String> {
        let server_printers = self.api_client.fetch_printers(token).await?;
        let db = self.db.lock().await;
        let mut count = 0u32;

        for sp in &server_printers {
            match PrintQueue::get_printer_by_server_id(&db, sp.id) {
                Ok(local) => {
                    let changed = local.ip_address.as_deref() != sp.ip_address.as_deref()
                        || local.port != sp.port
                        || local.connection_type != sp.connection_type
                        || local.hosted != sp.hosted;
                    if changed {
                        // الاستضافة من السيرفر يُحدَّث، واسم الطابعة/عرض الورق/مسار USB تبقى محفوظة محليًا
                        let updated = queue::SavedPrinter {
                            connection_type: sp.connection_type.clone(),
                            ip_address: sp.ip_address.clone(),
                            port: sp.port,
                            hosted: sp.hosted,
                            ..local
                        };
                        let _ = PrintQueue::save_printer(&db, &updated);
                    }
                    count += 1;
                }
                Err(_) => {
                    let new_printer = queue::SavedPrinter {
                        id: 0,
                        server_printer_id: Some(sp.id),
                        name: sp.name.clone(),
                        connection_type: sp.connection_type.clone(),
                        ip_address: sp.ip_address.clone(),
                        port: sp.port,
                        // من تعريف الوكيل المحفوظ على السيرفر (عرض الورق + مسار USB)
                        paper_width: sp.paper_width,
                        hosted: sp.hosted,
                        device_path: sp.device_path.clone(),
                        is_online: true,
                        last_test_at: None,
                        last_test_success: None,
                    };
                    if PrintQueue::save_printer(&db, &new_printer).is_ok() {
                        count += 1;
                    }
                }
            }
        }

        Ok(count)
    }

    /// معالجة الوظيفة التالية: رندر ESC/POS → إرسال للطابعة → الإبلاغ عن الحالة
    pub async fn process_next_job(&self, token: &str) -> Result<bool, String> {
        let db = self.db.lock().await;
        let pending = PrintQueue::get_pending(&db).map_err(|e| e.to_string())?;

        if pending.is_empty() {
            return Ok(false);
        }

        let job = &pending[0];
        let job_id = job.job_id;
        let payload_json = job.payload.clone();
        let printer_id = job.printer_id;
        let attempts = job.attempts;
        let max_attempts = job.max_attempts;

        // البحث عن الطابعة قبل أي إبلاغ (القفل ممسوك أصلًا من get_pending)
        // أولًا بمعرّفها في السيرفر، ثم بالمعرّف المحلي
        let printer = if let Some(pid) = printer_id {
            PrintQueue::get_printer_by_server_id(&db, pid)
                .ok()
                .or_else(|| PrintQueue::get_printer_by_id(&db, pid).ok())
        } else {
            None
        };

        // حارس الاستضافة (م3): لا يطبع هذا الجهاز إلا ما يستضيفه فعلًا.
        // لا نبلّغ السيرفر — الوظيفة تبقى pending لتصلك من جهازها الشرعي.
        if let Some(p) = &printer {
            if !queue::printer_is_hosted_here(p) {
                let _ = PrintQueue::update_status(
                    &db,
                    job_id,
                    JobStatus::Failed,
                    Some("طابعة غير مستضافة على هذا الجهاز — أُسقطت محليًا وستصل لجهازها الشرعي"),
                );
                let _ = AgentLogger::warn(
                    &db,
                    "printer",
                    &format!("الوظيفة #{}: الطابعة غير مستضافة على هذا الجهاز — أُسقطت وستصل لجهازها الشرعي", job_id),
                );
                log::warn!(
                    "job {} rejected by hosting guard: printer {:?} belongs to another station",
                    job_id,
                    p.server_printer_id
                );
                return Ok(true);
            }
        }

        // Mark as printing (محليًا + إبلاغ السيرفر: processing → printing)
        PrintQueue::update_status(&db, job_id, JobStatus::Printing, None).map_err(|e| e.to_string())?;
        PrintQueue::increment_attempts(&db, job_id).map_err(|e| e.to_string())?;
        let printer_label = printer.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| "طابعة غير محددة".to_string());
        let _ = AgentLogger::info(
            &db,
            "printer",
            &format!("جاري طباعة الوظيفة #{} على «{}» (محاولة {}/{})", job_id, printer_label, attempts + 1, max_attempts),
        );
        drop(db);

        let _ = self.api_client.update_job_status(token, job_id, "processing", None).await;

        // Render ESC/POS
        let paper_width = printer.as_ref().and_then(|p| p.paper_width).unwrap_or(80);
        let escpos_data = match PrintEngine::render_escpos(&payload_json, paper_width) {
            Ok(data) => data,
            Err(e) => {
                let db = self.db.lock().await;
                let _ = PrintQueue::update_status(&db, job_id, JobStatus::Failed, Some(&e));
                let _ = AgentLogger::error(&db, "printer", &format!("فشلت وظيفة #{} أثناء تجهيز الطباعة: {}", job_id, e), None);
                let _ = self.api_client.update_job_status(token, job_id, "failed", Some(&e)).await;
                return Ok(true);
            }
        };

        // Send to printer
        let print_result = match &printer {
            Some(p) => send_to_printer(p, &escpos_data).await,
            None => Err("لا توجد طابعة محلية مطابقة لهذه الوظيفة — تحقق من ربط الطابعات".to_string()),
        };

        // Update status
        let db = self.db.lock().await;
        match print_result {
            Ok(_) => {
                let _ = PrintQueue::update_status(&db, job_id, JobStatus::Printed, None);
                let _ = AgentLogger::info(&db, "printer", &format!("تمت طباعة الوظيفة #{} بنجاح", job_id));
                let _ = self.api_client.update_job_status(token, job_id, "completed", None).await;
                log::info!("Job {} printed successfully", job_id);
            }
            Err(e) => {
                if attempts + 1 >= max_attempts {
                    let _ = PrintQueue::update_status(&db, job_id, JobStatus::Failed, Some(&e));
                    let _ = AgentLogger::error(&db, "printer", &format!("فشلت وظيفة #{} بعد {} محاولات: {}", job_id, attempts + 1, e), None);
                    let _ = self.api_client.update_job_status(token, job_id, "failed", Some(&e)).await;
                    log::error!("Job {} failed after {} attempts: {}", job_id, attempts + 1, e);
                } else {
                    let _ = PrintQueue::update_status(&db, job_id, JobStatus::Queued, Some(&e));
                    let _ = AgentLogger::warn(&db, "printer", &format!("محاولة {}/{} للوظيفة #{} فشلت: {} — ستُعاد المحاولة", attempts + 1, max_attempts, job_id, e));
                    log::warn!("Job {} attempt {} failed, will retry: {}", job_id, attempts + 1, e);
                }
            }
        }

        Ok(true)
    }

    pub async fn report_status(&self, token: &str, job_id: i64, status: &str, error: Option<&str>) {
        let _ = self.api_client.update_job_status(token, job_id, status, error).await;
    }
}

/// إرسال البيانات إلى الطابعة حسب نوع الاتصال:
/// - شبكة → TCP 9100
/// - USB → المسار الصريح (device_path) إن حُدِّد، وإلا أول /dev/usb/lp* متاح
/// - بلوتوث → /dev/rfcomm0 (مرتبط سلفًا)
async fn send_to_printer(printer: &queue::SavedPrinter, data: &[u8]) -> Result<(), String> {
    let conn = printer.connection_type.to_lowercase();

    if conn == "usb" || conn == "bluetooth" {
        // تعيين صريح لجهاز USB (م3): يُفضَّل على البحث التلقائي «أول جهاز متاح»
        if conn == "usb" {
            if let Some(path) = printer.device_path.as_deref() {
                return write_raw_device(path, data).await.map_err(|e| {
                    format!("طابعة {} (المسار {}): {}", printer.name, path, e)
                });
            }
        }

        let devices: Vec<&str> = if conn == "bluetooth" {
            vec!["/dev/rfcomm0"]
        } else {
            vec!["/dev/usb/lp0", "/dev/usb/lp1", "/dev/usb/lp2"]
        };

        let target = devices
            .iter()
            .find(|d| std::path::Path::new(d).exists())
            .copied();

        return match target {
            Some(dev) => write_raw_device(dev, data).await,
            None => Err(format!(
                "تعذر الوصول إلى طابعة {} — لا يوجد جهاز خام متاح ({})",
                printer.name,
                devices.join(" أو ")
            )),
        };
    }

    // شبكة
    let (ip, port) = match (&printer.ip_address, printer.port) {
        (Some(ip), Some(port)) => (ip.clone(), port),
        _ => return Err(format!("طابعة الشبكة {} بلا عنوان IP أو منفذ", printer.name)),
    };
    send_to_network_printer(&ip, port, data).await
}

/// كتابة خام على ملف جهاز واحد (USB/بلوتوث)
async fn write_raw_device(dev: &str, data: &[u8]) -> Result<(), String> {
    let dev = dev.to_string();
    let data = data.to_vec();
    tokio::task::spawn_blocking(move || {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(&dev)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    format!(
                        "تعذر فتح {}: لا صلاحية (Permission denied) — {}",
                        dev,
                        crate::commands::USB_PERM_HINT
                    )
                } else {
                    format!("تعذر فتح {}: {}", dev, e)
                }
            })?;
        file.write_all(&data).map_err(|e| format!("تعذر الكتابة على {}: {}", dev, e))?;
        file.flush().map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Send raw bytes to a network printer (TCP port 9100)
async fn send_to_network_printer(ip: &str, port: u16, data: &[u8]) -> Result<(), String> {
    let addr = format!("{}:{}", ip, port);
    let mut stream = tokio::net::TcpStream::connect(&addr)
        .await
        .map_err(|e| format!("Cannot connect to printer {}: {}", addr, e))?;

    use tokio::io::AsyncWriteExt;
    stream.write_all(data)
        .await
        .map_err(|e| format!("Write to printer failed: {}", e))?;

    stream.flush().await.map_err(|e| format!("Flush failed: {}", e))?;

    Ok(())
}
