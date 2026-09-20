use std::sync::Arc;
use tokio::sync::Mutex;
use crate::queue::{self, PrintQueue, JobStatus};
use crate::printer::engine::PrintEngine;

pub struct SyncEngine {
    pub api_client: Arc<super::api::ApiClient>,
    pub db: Arc<Mutex<rusqlite::Connection>>,
}

impl SyncEngine {
    pub fn new(api_client: Arc<super::api::ApiClient>, db: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self { api_client, db }
    }

    /// Fetch pending jobs from server and add to local queue
    pub async fn sync_pending_jobs(&self, token: &str) -> Result<u32, String> {
        let jobs = self.api_client.fetch_pending_jobs(token).await?;
        let mut count = 0u32;

        let db = self.db.lock().await;
        for job in &jobs {
            let create = queue::CreateJob {
                job_id: job.job_id,
                store_id: job.store_id,
                printer_id: job.printer_id,
                section_id: job.section_id,
                payload: job.payload.clone(),
                max_attempts: 3,
            };
            if PrintQueue::enqueue(&db, &create).is_ok() {
                count += 1;
            }
        }

        Ok(count)
    }

    /// Process next job in queue: render ESC/POS → send to printer → report status
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

        // Mark as printing
        PrintQueue::update_status(&db, job_id, JobStatus::Printing, None).map_err(|e| e.to_string())?;
        PrintQueue::increment_attempts(&db, job_id).map_err(|e| e.to_string())?;
        drop(db);

        // Get printer info
        let printer = {
            let db = self.db.lock().await;
            if let Some(pid) = printer_id {
                PrintQueue::get_printer_by_id(&db, pid).ok()
            } else {
                None
            }
        };

        // Render ESC/POS
        let paper_width = printer.as_ref().and_then(|p| p.paper_width).unwrap_or(80);
        let escpos_data = match PrintEngine::render_escpos(&payload_json, paper_width) {
            Ok(data) => data,
            Err(e) => {
                let db = self.db.lock().await;
                let _ = PrintQueue::update_status(&db, job_id, JobStatus::Failed, Some(&e));
                let _ = self.api_client.update_job_status(token, job_id, "failed", Some(&e)).await;
                return Ok(true);
            }
        };

        // Send to printer
        let print_result = if let Some(ref printer) = printer {
            if let (Some(ip), Some(port)) = (&printer.ip_address, printer.port) {
                send_to_network_printer(ip, port, &escpos_data).await
            } else {
                Err("Printer missing IP/port".to_string())
            }
        } else {
            Err("No printer assigned".to_string())
        };

        // Update status
        let db = self.db.lock().await;
        match print_result {
            Ok(_) => {
                let _ = PrintQueue::update_status(&db, job_id, JobStatus::Printed, None);
                let _ = self.api_client.update_job_status(token, job_id, "completed", None).await;
                log::info!("Job {} printed successfully", job_id);
            }
            Err(e) => {
                if attempts + 1 >= max_attempts {
                    let _ = PrintQueue::update_status(&db, job_id, JobStatus::Failed, Some(&e));
                    let _ = self.api_client.update_job_status(token, job_id, "failed", Some(&e)).await;
                    log::error!("Job {} failed after {} attempts: {}", job_id, attempts + 1, e);
                } else {
                    let _ = PrintQueue::update_status(&db, job_id, JobStatus::Queued, Some(&e));
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
