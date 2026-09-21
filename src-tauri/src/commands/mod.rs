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
    pub server_url: String,
    pub machine_name: String,
    pub agent_id: Option<i64>,
    pub version: String,
    pub ws_status: String,
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

    Ok(DashboardData {
        paired: config.paired,
        store_name: config.store_name.clone(),
        server_url: config.server_url.clone(),
        machine_name: hostname::get().map(|h| h.to_string_lossy().to_string()).unwrap_or_default(),
        agent_id: config.agent_id,
        version: config.version.clone(),
        ws_status: ws_status.label().to_string(),
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
    let config = app.config.lock().await;

    let machine_name = hostname::get().map(|h| h.to_string_lossy().to_string()).unwrap_or_default();

    let response = app.api_client.pair(
        &token,
        &machine_name,
        &config.machine_id,
    ).await?;

    if response.success {
        if let Some(tok) = &response.token {
            app.security_store.save_token(tok)?;
        }
        drop(config);
        let mut config = app.config.lock().await;
        config.paired = true;
        config.agent_id = response.agent_id;
        config.store_id = response.store_id;
        config.store_name = Some("متجر".to_string());
        config.agent_token = response.token.clone();
        let _ = app.security_store.save_config(&config);
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
    app.security_store.clear_token()?;
    app.ws_manager.lock().await.disconnect().await;

    let mut config = app.config.lock().await;
    config.paired = false;
    config.agent_id = None;
    config.store_id = None;
    config.store_name = None;
    config.agent_token = None;
    let _ = app.security_store.save_config(&config);

    let db = app.db.lock().await;
    let _ = AgentLogger::info(&db, "pairing", "Agent unpaired");

    Ok("تم فك الربط بنجاح".to_string())
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
