use crate::AgentConfig;
use crate::queue::{self, JobStatus, SavedPrinter};
use crate::logs::AgentLogger;
use crate::printer::manager::PrinterManager;
use crate::pairing::PairingResponse;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub paired: bool,
    pub store_name: Option<String>,
    /// اسم المحطة كما هو مسجّل على السيرفر (يظهر بدل اسم عام «متجر»)
    #[serde(default)]
    pub station_name: Option<String>,
    pub server_url: String,
    pub machine_name: String,
    pub agent_id: Option<i64>,
    pub version: String,
    pub ws_status: String,
    /// الاتصال الحقيقي بالسيرفر (نجاح HTTP خلال نافذة النبض) — لا يعتمد على WS
    pub connected: bool,
    pub printers: Vec<SavedPrinter>,
    pub queue_stats: queue::QueueStats,
    pub last_job: Option<queue::PrintJob>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: i64,
    pub level: String,
    pub category: String,
    pub message: String,
    pub details: Option<String>,
    pub created_at: String,
}

// --- Dashboard ---

#[tauri::command]
pub async fn get_dashboard(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
) -> Result<DashboardData, String> {
    let app = state.lock().await;
    let db = app.db.lock().await;

    let printers = queue::PrintQueue::get_printers(&db).unwrap_or_default();
    let queue_stats = queue::PrintQueue::stats(&db).map_err(|e| e.to_string())?;

    let last_job = {
        let mut stmt = db.prepare(
            "SELECT id, job_id, store_id, printer_id, section_id, status, payload, attempts, max_attempts, last_error, created_at, updated_at
             FROM print_queue ORDER BY created_at DESC LIMIT 1"
        ).map_err(|e| e.to_string())?;
        let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
        if let Some(row) = rows.next().map_err(|e| e.to_string())? {
            Some(queue::PrintJob {
                id: row.get(0).map_err(|e| e.to_string())?,
                job_id: row.get(1).map_err(|e| e.to_string())?,
                store_id: row.get(2).map_err(|e| e.to_string())?,
                printer_id: row.get(3).map_err(|e| e.to_string())?,
                section_id: row.get(4).map_err(|e| e.to_string())?,
                status: JobStatus::from_str(&row.get::<_, String>(5).map_err(|e| e.to_string())?),
                payload: row.get(6).map_err(|e| e.to_string())?,
                attempts: row.get::<_, u32>(7).map_err(|e| e.to_string())?,
                max_attempts: row.get::<_, u32>(8).map_err(|e| e.to_string())?,
                last_error: row.get(9).map_err(|e| e.to_string())?,
                created_at: row.get(10).map_err(|e| e.to_string())?,
                updated_at: row.get(11).map_err(|e| e.to_string())?,
            })
        } else { None }
    };

    let ws_status = app.ws_manager.lock().await.get_status().await;
    let config = app.config.lock().await;

    // «حالة الاتصال» = نجاح أي نداء HTTP حديث (الـ WS لا يتصل في بيئة التطوير)
    let connected = config.paired && app.api_client.http_connected();

    Ok(DashboardData {
        paired: config.paired,
        store_name: config.store_name.clone(),
        station_name: config.station_name.clone(),
        server_url: config.server_url.clone(),
        machine_name: hostname::get().map(|h| h.to_string_lossy().to_string()).unwrap_or_default(),
        agent_id: config.agent_id,
        version: config.version.clone(),
        ws_status: ws_status.label().to_string(),
        connected,
        printers,
        queue_stats,
        last_job,
    })
}

// --- Pairing ---

#[tauri::command]
pub async fn start_pairing(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
    token: String,
) -> Result<PairingResponse, String> {
    let app = state.lock().await;
    do_pairing(&app, token).await
}

/// نواة الربط — تُستدعى من أمر الإدخال اليدوي ومن الرابط العميق
pub async fn do_pairing(app: &crate::AppState, token: String) -> Result<PairingResponse, String> {
    let machine_name = hostname::get().map(|h| h.to_string_lossy().to_string()).unwrap_or_default();

    let response = {
        let config = app.config.lock().await;
        app.api_client.pair(
            &token,
            &machine_name,
            &config.machine_id,
        ).await?
    };

    if response.success {
        if let Some(tok) = &response.token {
            app.security_store.save_token(tok)?;
        }
        {
            let mut config = app.config.lock().await;
            config.paired = true;
            config.agent_id = response.agent_id;
            config.store_id = response.store_id;
            config.store_name = Some("متجر".to_string());
            // اسم المحطة الحقيقي من السيرفر — يظهر بدل اسم عام
            config.station_name = response.station_name.clone().filter(|s| !s.trim().is_empty());
            config.agent_token = response.token.clone();
            let _ = app.security_store.save_config(&config);
        }

        // Auto-connect WebSocket after pairing
        if let (Some(tok), Some(sid)) = (&response.token, response.store_id) {
            let _ = app.ws_manager.lock().await.connect(tok, sid).await;
        }
    }

    {
        let db = app.db.lock().await;
        let msg = if response.success { "Pairing successful" } else { response.message.as_deref().unwrap_or("Pairing failed") };
        let _ = AgentLogger::info(&db, "pairing", msg);
    }

    Ok(response)
}

// --- Queue ---

#[tauri::command]
pub async fn get_queue_jobs(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
) -> Result<Vec<queue::PrintJob>, String> {
    let app = state.lock().await;
    let db = app.db.lock().await;
    queue::PrintQueue::get_all(&db).map_err(|e| e.to_string())
}

// --- Logs ---

#[tauri::command]
pub async fn get_logs(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
    limit: u32,
) -> Result<Vec<LogEntry>, String> {
    let app = state.lock().await;
    let db = app.db.lock().await;
    let logs = AgentLogger::get_recent(&db, limit).map_err(|e| e.to_string())?;
    Ok(logs.into_iter().map(|l| LogEntry {
        id: l.id, level: l.level, category: l.category, message: l.message,
        details: l.details, created_at: l.created_at,
    }).collect())
}

// --- Config ---

#[tauri::command]
pub async fn get_config(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
) -> Result<AgentConfig, String> {
    let app = state.lock().await;
    let config = app.config.lock().await;
    Ok(config.clone())
}

#[tauri::command]
pub async fn reconnect(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
) -> Result<String, String> {
    let app = state.lock().await;
    let config = app.config.lock().await;

    if let Some(token) = &config.agent_token {
        let store_id = config.store_id.unwrap_or(0);
        let token_clone = token.clone();
        drop(config);
        let mut ws = app.ws_manager.lock().await;
        ws.connect(&token_clone, store_id).await?;
        Ok("تم إعادة الاتصال بنجاح".to_string())
    } else {
        Err("الجهاز غير مربوط".to_string())
    }
}

#[tauri::command]
pub async fn unpair(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
) -> Result<String, String> {
    let app = state.lock().await;
    local_unpair(&app).await?;

    let db = app.db.lock().await;
    let _ = AgentLogger::info(&db, "pairing", "Agent unpaired");

    Ok("تم فك الربط بنجاح".to_string())
}

/// فك الربط المحلي فقط (بدون نداء للسيرفر)
pub async fn local_unpair(app: &crate::AppState) -> Result<(), String> {
    app.security_store.clear_token()?;
    app.ws_manager.lock().await.disconnect().await;

    let mut config = app.config.lock().await;
    config.paired = false;
    config.agent_id = None;
    config.store_id = None;
    config.store_name = None;
    config.station_name = None;
    config.agent_token = None;
    config.session_user = None;
    config.session_user_id = None;
    let _ = app.security_store.save_config(&config);

    Ok(())
}

// --- تبديل سيرفر الاختبار ---

/// تطبيق عنوان سيرفر جديد: التحقق + اشتقاق ws + تجديد عميل الشبكة وقناة WS + الحفظ
pub async fn apply_server_url(app: &mut crate::AppState, raw: String) -> Result<(), String> {
    let url = raw.trim().trim_end_matches('/').to_string();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("عنوان السيرفر يجب أن يبدأ بـ http:// أو https://".to_string());
    }

    let ws_url = if let Some(rest) = url.strip_prefix("https://") {
        format!("wss://{}", rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        format!("ws://{}", rest)
    } else {
        return Err("عنوان سيرفر غير صالح".to_string());
    };

    {
        let mut config = app.config.lock().await;
        config.server_url = url.clone();
        config.ws_url = ws_url.clone();
        let _ = app.security_store.save_config(&config);
    }

    // تجديد المبنيَّين عند الإقلاع
    app.api_client = Arc::new(crate::api::ApiClient::new(url));
    app.ws_manager.lock().await.ws_url = ws_url;

    Ok(())
}

/// تغيير عنوان السيرفر من الإعدادات — فقط عندما يكون البرنامج غير مربوط
#[tauri::command]
pub async fn set_server_url(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
    url: String,
) -> Result<serde_json::Value, String> {
    let mut app = state.lock().await;

    if app.config.lock().await.paired {
        return Err("البرنامج مرتبط بسيرفر — أزل الربط أولًا لتغيير العنوان".to_string());
    }

    apply_server_url(&mut app, url).await?;

    {
        let db = app.db.lock().await;
        let _ = AgentLogger::info(&db, "server", "Server URL changed from settings");
    }

    let config = app.config.lock().await;
    Ok(serde_json::json!({
        "server_url": config.server_url,
        "ws_url": config.ws_url,
    }))
}

/// استقبال رابط عميق `ordoo-agent://pair?server=...&token=...`:
/// تبديل السيرفر (إن تغيّر، مع فك الربط المحلي) ثم الربط بالتوكن مباشرة
pub async fn deep_link_connect(
    state: Arc<Mutex<crate::AppState>>,
    server: Option<String>,
    token: Option<String>,
) -> Result<String, String> {
    let mut app = state.lock().await;

    if let Some(server) = server {
        let server = server.trim().trim_end_matches('/').to_string();
        let (current, paired) = {
            let config = app.config.lock().await;
            (config.server_url.trim_end_matches('/').to_string(), config.paired)
        };

        if server != current {
            if paired {
                local_unpair(&app).await?;
            }
            apply_server_url(&mut app, server).await?;

            let db = app.db.lock().await;
            let _ = AgentLogger::info(&db, "deeplink", "Server switched by web deep link");
        }
    }

    match token {
        Some(token) => {
            let response = do_pairing(&app, token).await?;
            if response.success {
                let name = response.station_name.unwrap_or_else(|| "المحطة".to_string());
                let db = app.db.lock().await;
                let _ = AgentLogger::info(&db, "deeplink", &format!("Paired via deep link: {}", name));
                Ok(format!("تم الربط بنجاح — المحطة «{}»", name))
            } else {
                Err(response.message.unwrap_or_else(|| "فشل الربط".to_string()))
            }
        }
        None => Ok("تم تحديث عنوان السيرفر".to_string()),
    }
}

// --- Printers ---

#[tauri::command]
pub async fn get_printers(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
) -> Result<Vec<SavedPrinter>, String> {
    let app = state.lock().await;
    let db = app.db.lock().await;
    queue::PrintQueue::get_printers(&db).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_printer(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
    name: String,
    connection_type: String,
    ip_address: Option<String>,
    port: Option<u16>,
    paper_width: Option<u32>,
) -> Result<SavedPrinter, String> {
    let app = state.lock().await;
    let db = app.db.lock().await;

    let printer = SavedPrinter {
        id: 0,
        server_printer_id: None,
        name,
        connection_type,
        ip_address,
        port,
        paper_width: Some(paper_width.unwrap_or(80)),
        hosted: None,
        device_path: None,
        is_online: false,
        last_test_at: None,
        last_test_success: None,
    };

    queue::PrintQueue::save_printer(&db, &printer).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_printer(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
    id: i64,
) -> Result<(), String> {
    let app = state.lock().await;
    let db = app.db.lock().await;
    queue::PrintQueue::delete_printer(&db, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn test_printer(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
    id: i64,
) -> Result<bool, String> {
    let app = state.lock().await;
    let db = app.db.lock().await;

    let printer = queue::PrintQueue::get_printer_by_id(&db, id).map_err(|e| e.to_string())?;

    let ip = printer.ip_address.ok_or("Printer has no IP")?;
    let port = printer.port.ok_or("Printer has no port")?;

    // Try connecting
    let addr = format!("{}:{}", ip, port);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::net::TcpStream::connect(&addr),
    ).await;

    let success = result.is_ok() && result.unwrap().is_ok();

    let _ = queue::PrintQueue::update_printer_status(&db, id, success, Some(success));

    let msg = if success { "Test successful" } else { "Test failed" };
    let _ = AgentLogger::log(&db, if success { "info" } else { "error" }, "printer", msg, None);

    Ok(success)
}

#[tauri::command]
pub async fn discover_printers(
    _state: State<'_, Arc<Mutex<crate::AppState>>>,
    subnet: String,
) -> Result<Vec<crate::printer::manager::DiscoveredPrinter>, String> {
    Ok(PrinterManager::discover_network_printers(&subnet).await)
}

/// المسارات الخام المتاحة لطابعات USB على هذا الجهاز (/dev/usb/lp*)
#[tauri::command]
pub async fn list_usb_nodes() -> Result<Vec<String>, String> {
    let mut nodes: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/dev/usb") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("lp") {
                nodes.push(format!("/dev/usb/{}", name));
            }
        }
    }
    nodes.sort();
    Ok(nodes)
}

/// تعيين جهاز USB صريح لطابعة (يُفضَّل على «أول جهاز متاح») — م3
#[tauri::command]
pub async fn set_printer_device_path(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
    id: i64,
    device_path: Option<String>,
) -> Result<(), String> {
    let app = state.lock().await;
    let db = app.db.lock().await;
    queue::PrintQueue::set_printer_device_path(&db, id, device_path.as_deref())
        .map_err(|e| e.to_string())
}

/// عرض عرض الورق لكل طابعة (58/80) — م3
#[tauri::command]
pub async fn set_printer_paper_width(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
    id: i64,
    paper_width: u32,
) -> Result<(), String> {
    if paper_width != 58 && paper_width != 80 {
        return Err("عرض الورق المدعوم: 58 أو 80".to_string());
    }
    let app = state.lock().await;
    let db = app.db.lock().await;
    queue::PrintQueue::set_printer_paper_width(&db, id, paper_width)
        .map_err(|e| e.to_string())
}

// --- تعريف الطابعات من هذا البرنامج (USB أو شبكة فقط) ---

/// أمر إصلاح صلاحيات أجهزة الطابعة الخام (root:lp) — يُقترح عند رفض الفتح
/// ملاحظة: بعض الأنوية تُظهر الجهاز بـ SUBSYSTEM=usbmisc لا lp — لذا المطابقة بالاسم فقط
pub const USB_PERM_HINT: &str = "أضِف إذن الوصول ثم أعد الاختبار: echo 'KERNEL==\"lp[0-9]*\", MODE=\"0666\"' | sudo tee /etc/udev/rules.d/99-ordoo-printer.rules && sudo udevadm control --reload && sudo udevadm trigger --subsystem-match=usbmisc";

/// جلب إعداد الطباعة: طابعة الفاتورة + كل قسم وطابعته
#[tauri::command]
pub async fn get_print_setup(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
) -> Result<crate::api::PrintSetup, String> {
    let app = state.lock().await;
    let token = { app.config.lock().await.agent_token.clone() }
        .ok_or("الجهاز غير مربوط — أكمل الربط أولًا")?;
    let setup = app.api_client.get_print_setup(&token).await?;
    sync_local_from_setup(&app, &setup).await;
    Ok(setup)
}

/// حفظ طابعة الفاتورة على السيرفر ثم نسخها فورًا إلى الطابور المحلي
#[tauri::command]
pub async fn save_receipt_printer(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
    printer: Option<crate::api::PrinterConfig>,
) -> Result<crate::api::PrintSetup, String> {
    let app = state.lock().await;
    let token = { app.config.lock().await.agent_token.clone() }
        .ok_or("الجهاز غير مربوط — أكمل الربط أولًا")?;
    let setup = app
        .api_client
        .save_receipt_printer(&token, printer.as_ref())
        .await?;
    sync_local_from_setup(&app, &setup).await;
    Ok(setup)
}

/// حفظ طابعة قسم على السيرفر ثم نسخها فورًا إلى الطابور المحلي
#[tauri::command]
pub async fn save_section_printer(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
    section_id: i64,
    printer: crate::api::PrinterConfig,
) -> Result<crate::api::PrintSetup, String> {
    let app = state.lock().await;
    let token = { app.config.lock().await.agent_token.clone() }
        .ok_or("الجهاز غير مربوط — أكمل الربط أولًا")?;
    let setup = app
        .api_client
        .save_section_printer(&token, section_id, &printer)
        .await?;
    sync_local_from_setup(&app, &setup).await;
    Ok(setup)
}

/// اختبار طابعة من نموذج التعريف قبل الحفظ:
/// شبكة → اتصال TCP بالـIP والبورت؛ USB → فتح الجهاز (توفر + صلاحيات)
#[tauri::command]
pub async fn test_printer_config(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
    connection_type: String,
    ip_address: Option<String>,
    port: Option<u16>,
    device_path: Option<String>,
) -> Result<serde_json::Value, String> {
    let result = if connection_type == "usb" {
        // الجهاز المحدد صراحةً، وإلا أول /dev/usb/lp* متاح
        let nodes = list_usb_nodes().await.unwrap_or_default();
        let path = device_path
            .filter(|p| !p.is_empty())
            .or_else(|| nodes.first().cloned())
            .ok_or("لا توجد أجهزة USB متاحة — وصّل الطابعة ثم أعد الاختبار")?;

        // فتح للكتابة فقط — مطابق تمامًا لمسار الطباعة الفعلي (write_raw_device)
        match std::fs::OpenOptions::new().write(true).open(&path) {
            Ok(_) => serde_json::json!({
                "success": true,
                "message": format!("تم فتح {} للكتابة بنجاح ✓", path),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => serde_json::json!({
                "success": false,
                "message": format!(
                    "تعذر فتح {}: لا صلاحية (Permission denied) — الجهاز مملوك لـ root:lp وأنت خارجها.\n{}",
                    path, USB_PERM_HINT
                ),
            }),
            Err(e) => serde_json::json!({
                "success": false,
                "message": format!("تعذر فتح {}: {} — تحقق من توصيل الطابعة", path, e),
            }),
        }
    } else {
        let ip = ip_address
            .filter(|s| !s.is_empty())
            .ok_or("طابعة الشبكة تتطلب عنوان IP")?;
        let port = port.unwrap_or(9100);
        let addr = format!("{}:{}", ip, port);

        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::net::TcpStream::connect(&addr),
        )
        .await
        {
            Ok(Ok(_)) => serde_json::json!({
                "success": true,
                "message": format!("الاتصال بـ{}:{} ناجح ✓", ip, port),
            }),
            Ok(Err(e)) => serde_json::json!({
                "success": false,
                "message": format!("تعذر الاتصال بـ{}:{} — {}", ip, port, e),
            }),
            Err(_) => serde_json::json!({
                "success": false,
                "message": format!("انتهت مهلة الاتصال بـ{}:{}", ip, port),
            }),
        }
    };

    let ok = result["success"].as_bool().unwrap_or(false);
    let msg = result["message"].as_str().unwrap_or("").to_string();
    {
        let app = state.lock().await;
        let db = app.db.lock().await;
        let _ = AgentLogger::log(
            &db,
            if ok { "info" } else { "error" },
            "printer",
            &format!("اختبار الطابعة: {}", msg),
            None,
        );
    }

    Ok(result)
}

/// نسخ التعريفات من استجابة السيرفر إلى جدول الطابعات المحلي
/// (بلا انتظار دورة المرآة — أول وظيفة طباعة بعد الحفظ تجد طابعتها)
async fn sync_local_from_setup(app: &crate::AppState, setup: &crate::api::PrintSetup) {
    let db = app.db.lock().await;
    for cfg in setup
        .receipt_printer
        .iter()
        .chain(setup.sections.iter().filter_map(|s| s.printer.as_ref()))
    {
        if let Some(server_id) = cfg.id {
            upsert_local_printer(&db, server_id, cfg);
        }
    }
}

fn upsert_local_printer(db: &rusqlite::Connection, server_id: i64, cfg: &crate::api::PrinterConfig) {
    // محليًا تُخزَّن الشبكة wifi (مطابق لتخزين السيرفر — لا هَجْس في المرآة)
    let connection_type = if cfg.connection_type == "usb" { "usb" } else { "wifi" };
    let printer = SavedPrinter {
        id: 0,
        server_printer_id: Some(server_id),
        name: cfg.name.clone(),
        connection_type: connection_type.to_string(),
        ip_address: cfg.ip_address.clone(),
        port: cfg.port.or(Some(9100)),
        paper_width: Some(cfg.paper_width.unwrap_or(80)),
        hosted: Some(true),
        device_path: cfg.device_path.clone(),
        is_online: true,
        last_test_at: None,
        last_test_success: None,
    };

    match queue::PrintQueue::get_printer_by_server_id(db, server_id) {
        Ok(existing) => {
            let updated = SavedPrinter { id: existing.id, ..printer };
            let _ = queue::PrintQueue::save_printer(db, &updated);
        }
        Err(_) => {
            let _ = queue::PrintQueue::save_printer(db, &printer);
        }
    }
}

// --- Server API commands ---

#[tauri::command]
pub async fn get_server_printers(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
) -> Result<serde_json::Value, String> {
    let app = state.lock().await;
    let config = app.config.lock().await;
    let token = config.agent_token.as_deref().ok_or("Not paired")?;
    let printers = app.api_client.fetch_printers(token).await?;
    Ok(serde_json::json!({
        "success": true,
        "printers": printers,
    }))
}

#[tauri::command]
pub async fn test_server_printer(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
    printer_id: i64,
) -> Result<crate::api::PrinterTestResult, String> {
    let app = state.lock().await;
    let config = app.config.lock().await;
    let token = config.agent_token.as_deref().ok_or("Not paired")?;
    app.api_client.test_printer(token, printer_id).await
}

#[tauri::command]
pub async fn unpair_with_server(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
) -> Result<String, String> {
    let app = state.lock().await;

    // نداء السيرفر أولًا (يحذف المحطة من لوحة التاجر) — ثم فك الربط المحلي
    // في كل الأحوال، حتى لو تعذّر الاتصال
    let token = { app.config.lock().await.agent_token.clone() };
    let server_result = match &token {
        Some(t) => Some(app.api_client.unpair_from_server(t).await),
        None => None,
    };

    // local_unpair: يمسح التوكن + الجلسة (session_user) + يفصل WS ويطبّق paired=false
    local_unpair(&app).await?;

    let db = app.db.lock().await;
    let _ = AgentLogger::info(&db, "pairing", "Agent unpaired by user");

    match server_result {
        Some(Ok(())) => Ok("تم فك الربط وحذف المحطة من السيرفر".to_string()),
        Some(Err(e)) => Ok(format!(
            "تم فك الربط محلياً (تعذر الاتصال بالسيرفر: {})",
            e
        )),
        None => Ok("تم فك الربط".to_string()),
    }
}

#[tauri::command]
pub async fn check_connection(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
) -> Result<serde_json::Value, String> {
    let app = state.lock().await;
    let config = app.config.lock().await;
    let token = config.agent_token.as_deref().ok_or("الجهاز غير مربوط")?;

    match app.api_client.check_connection(token).await {
        Ok(true) => Ok(serde_json::json!({
            "success": true,
            "message": "الاتصال بالسيرفر يعمل بشكل طبيعي ✓"
        })),
        Ok(false) | Err(_) => Ok(serde_json::json!({
            "success": false,
            "message": "تعذر الاتصال بالسيرفر"
        })),
    }
}

// --- Update Check ---

#[tauri::command]
pub async fn check_for_updates(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
) -> Result<serde_json::Value, String> {
    let app = state.lock().await;
    let current_version = &app.config.lock().await.version;

    match app.api_client.check_version().await {
        Ok(server) => {
            // «أكبر من» فقط — مقارنة «مختلف عن» كانت تُظهر تنبيه إصدار جديد
            // بمعرِّف أقدم (كانت v0.3.2 تظهر على أجهزة تعمل بـ0.3.4)
            let has_update = is_version_newer(&server.version, current_version);
            Ok(serde_json::json!({
                "has_update": has_update,
                "current_version": current_version,
                "latest_version": server.version,
                "download_url": server.download_url,
                "changelog": server.changelog,
            }))
        }
        Err(_) => Ok(serde_json::json!({
            "has_update": false,
            "current_version": current_version,
            "latest_version": current_version,
        }))
    }
}

/// هل الإصدار المعلن أحدث فعلًا من الحالي؟ (أول فرق بين الأجزاء يحسم)
fn is_version_newer(latest: &str, current: &str) -> bool {
    fn parts(v: &str) -> Vec<u64> {
        v.trim()
            .trim_start_matches('v')
            .split(['.', '-', '+'])
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect()
    }

    let l = parts(latest);
    let c = parts(current);

    for i in 0..l.len().max(c.len()) {
        let a = l.get(i).copied().unwrap_or(0);
        let b = c.get(i).copied().unwrap_or(0);
        if a != b {
            return a > b;
        }
    }

    false
}

#[cfg(test)]
mod update_compare_tests {
    use super::is_version_newer;

    #[test]
    fn newer_version_is_an_update() {
        assert!(is_version_newer("0.3.5", "0.3.4"));
        assert!(is_version_newer("v0.4.0", "0.3.9"));
        assert!(is_version_newer("0.3.10", "0.3.9"));
    }

    #[test]
    fn older_or_equal_is_not_an_update() {
        assert!(!is_version_newer("0.3.2", "0.3.4")); // جذر المشكلة: معرِّف أقدم لا يُعلن تحديثًا
        assert!(!is_version_newer("0.3.4", "0.3.4"));
        assert!(!is_version_newer("0.4", "0.4.0"));
    }

    #[test]
    fn first_divergence_decides() {
        // 0.3.2 أقدم من 0.4.0 رغم أن الجزء الأخير (2) أكبر من (0)
        assert!(!is_version_newer("0.3.2", "0.4.0"));
        assert!(is_version_newer("0.4.0", "0.3.2"));
    }
}

// --- USB Printer Detection ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbPrinter {
    pub name: String,
    pub connection_type: String,
}

#[tauri::command]
pub async fn detect_usb_printers() -> Result<Vec<UsbPrinter>, String> {
    use std::process::Command;

    let output = Command::new("lsusb")
        .output()
        .map_err(|e| format!("Failed to run lsusb: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut printers = Vec::new();

    for line in stdout.lines() {
        let lower = line.to_lowercase();
        // Common printer vendor IDs
        if lower.contains("printer") || lower.contains("epson") || lower.contains("hp")
            || lower.contains("canon") || lower.contains("brother") || lower.contains("ricoh")
            || lower.contains("sharp") || lower.contains("kyocera") || lower.contains("samsung")
            || lower.contains("xerox") || lower.contains("oki") || lower.contains("citizen")
            || lower.contains("star") || lower.contains("bixolon") {
            printers.push(UsbPrinter {
                name: line.trim().to_string(),
                connection_type: "usb".to_string(),
            });
        }
    }

    Ok(printers)
}

// --- تسجيل الدخول من الوكيل (رابط مؤقت يُفتح في متصفح هذا الجهاز) ---

#[tauri::command]
pub async fn agent_login(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
    email: String,
    password: String,
) -> Result<serde_json::Value, String> {
    // استخراج التوكن والعميل ثم إفلات القفل قبل الاتصال بالشبكة
    let (api_client, token) = {
        let app = state.lock().await;
        let token = app.config.lock().await.agent_token.clone()
            .ok_or("البرنامج غير مربوط — اربط المحطة أولًا من التوكن")?;
        (app.api_client.clone(), token)
    };

    let response = api_client.login(&token, &email, &password).await?;

    // سجل محلي — لا نسجّل كلمة المرور أبدًا
    {
        let app = state.lock().await;
        let db = app.db.lock().await;
        let _ = AgentLogger::info(&db, "login", "تم طلب رابط تسجيل الدخول من الوكيل");
    }

    let url = response.url.ok_or("لم يُعِد السيرفر رابطًا")?;
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("رابط دخول غير صالح".to_string());
    }

    Ok(serde_json::json!({
        "success": true,
        "url": url,
        "station": response.station,
        "user": response.user,
        "expires_in": response.expires_in,
    }))
}

#[tauri::command]
#[allow(deprecated)]
pub async fn open_external(app: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_shell::ShellExt;
    app.shell().open(url, None).map_err(|e| e.to_string())
}

// --- جلسة المستخدم على الجهاز (اسم المستخدم + خروج متبادل مع الويب) ---

/// خروج من البرنامج: يلغي الجلسة على السيرفر (وتسقط جلسة الويب المرتبطة
/// عبر وسيط agent.session في طلبها التالي) ثم يمسح الاسم محليًا.
#[tauri::command]
pub async fn agent_session_logout(
    state: State<'_, Arc<Mutex<crate::AppState>>>,
) -> Result<String, String> {
    // استخراج التوكن والعميل ثم إفلات القفل قبل الاتصال بالشبكة
    let (api_client, token) = {
        let app = state.lock().await;
        let token = app.config.lock().await.agent_token.clone()
            .ok_or("البرنامج غير مربوط")?;
        (app.api_client.clone(), token)
    };

    let message = api_client.session_logout(&token).await?;

    {
        let app = state.lock().await;
        {
            let mut config = app.config.lock().await;
            config.session_user = None;
            config.session_user_id = None;
            let _ = app.security_store.save_config(&config);
        }
        let db = app.db.lock().await;
        let _ = AgentLogger::info(&db, "session", "تسجيل الخروج من البرنامج");
    }

    Ok(message)
}

/// دخول من رابط عميق `ordoo-agent://login?server=...&token=...`:
/// استهلاك توكن الويب لمرة واحدة — بشرط أن يكون الجهاز مربوطًا (كما طلب المستخدم:
/// «بشرط يكون الجهاز مسجل لدى المتجر ومربوط على Ordoo Agent»).
pub async fn deep_link_session_login(
    state: Arc<Mutex<crate::AppState>>,
    server: Option<String>,
    token: Option<String>,
) -> Result<String, String> {
    let token = token.ok_or("رابط الدخول يفتقد التوكن")?;

    let app = state.lock().await;

    let (api_client, agent_token) = {
        let config = app.config.lock().await;
        if !config.paired {
            return Err("البرنامج غير مربوط — اربط المحطة أولًا".to_string());
        }
        // لا نبدّل السيرفر في مسار الدخول: التبديل يُفك الربط ويُبطل توكن المحطة
        if let Some(server) = &server {
            let server = server.trim().trim_end_matches('/');
            if server != config.server_url.trim_end_matches('/') {
                return Err("رابط الدخول موجّه لسيرفر آخر من السيرفر المربوط به".to_string());
            }
        }
        (app.api_client.clone(), config.agent_token.clone().ok_or("البرنامج غير مربوط")?)
    };

    let response = api_client.session_login(&agent_token, &token).await?;

    let user = response.user.clone().unwrap_or_else(|| "مستخدم".to_string());
    {
        let mut config = app.config.lock().await;
        config.session_user = response.user;
        config.session_user_id = response.user_id;
        let _ = app.security_store.save_config(&config);
    }

    let db = app.db.lock().await;
    let _ = AgentLogger::info(&db, "session", &format!("تسجيل دخول من الويب: {}", user));

    Ok(format!("تم تسجيل الدخول في البرنامج باسم «{}»", user))
}
