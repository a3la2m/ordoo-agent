pub mod identity;
pub mod pairing;
pub mod printer;
pub mod queue;
pub mod websocket;
pub mod api;
pub mod sync;
pub mod security;
pub mod logs;
pub mod commands;

use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tauri::Manager;
use tauri::tray::TrayIconBuilder;
use identity::AgentIdentity;
use api::ApiClient;
use websocket::WsManager;
use queue::PrintQueue;
use printer::PrinterManager;
use security::SecureStore;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentConfig {
    pub store_id: Option<i64>,
    pub store_name: Option<String>,
    pub agent_id: Option<i64>,
    pub agent_token: Option<String>,
    pub server_url: String,
    pub ws_url: String,
    pub paired: bool,
    pub machine_id: String,
    pub installation_id: String,
    pub version: String,
    pub max_retries: u32,
    pub heartbeat_interval_secs: u64,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            store_id: None,
            store_name: None,
            agent_id: None,
            agent_token: None,
            server_url: "https://ordoo.shop".to_string(),
            ws_url: "wss://ordoo.shop".to_string(),
            paired: false,
            machine_id: String::new(),
            installation_id: String::new(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            max_retries: 3,
            heartbeat_interval_secs: 30,
        }
    }
}

pub struct AppState {
    pub db: Arc<Mutex<rusqlite::Connection>>,
    pub identity: AgentIdentity,
    pub api_client: Arc<ApiClient>,
    pub ws_manager: Mutex<WsManager>,
    pub print_queue: PrintQueue,
    pub printer_manager: Mutex<PrinterManager>,
    pub config: Arc<Mutex<AgentConfig>>,
    pub security_store: SecureStore,
    pub shutdown_sender: Option<mpsc::Sender<()>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Init database
            let app_data_dir = app.path().app_data_dir().expect("Failed to get app data dir");
            std::fs::create_dir_all(&app_data_dir).ok();
            let db_path = app_data_dir.join("ordoo_agent.db");
            let conn = rusqlite::Connection::open(&db_path).expect("Failed to open database");

            // Create tables
            queue::init_queue_db(&conn).expect("Failed to init queue DB");
            logs::init_logs_db(&conn).expect("Failed to init logs DB");

            // Init security store
            let security_store = SecureStore::new("ordoo-agent");

            // Load or create identity
            let identity = AgentIdentity::load_or_create(&security_store);

            // Load config from disk or create default
            let mut config = security_store.load_config().unwrap_or_else(|| {
                let mut c = AgentConfig::default();
                c.machine_id = identity.machine_id.clone();
                c.installation_id = identity.installation_id.clone();
                c
            });

            // Ensure identity fields are set
            config.machine_id = identity.machine_id.clone();
            config.installation_id = identity.installation_id.clone();

            // Try to load saved token
            if let Some(token) = security_store.load_token() {
                config.agent_token = Some(token);
                config.paired = true;
            }

            // Log startup
            {
                let _ = logs::AgentLogger::info(&conn, "system", &format!("Ordoo Agent v{} started", config.version));
            }

            // Init services
            let server_url = config.server_url.clone();
            let ws_url = config.ws_url.clone();
            let api_client = Arc::new(ApiClient::new(server_url));
            let ws_manager = WsManager::new(ws_url);
            let print_queue = PrintQueue;
            let printer_manager = PrinterManager::new();

            // Create shared state
            let state = Arc::new(Mutex::new(AppState {
                db: Arc::new(Mutex::new(conn)),
                identity,
                api_client,
                ws_manager: Mutex::new(ws_manager),
                print_queue,
                printer_manager: Mutex::new(printer_manager),
                config: Arc::new(Mutex::new(config.clone())),
                security_store,
                shutdown_sender: None,
            }));

            app.manage(state.clone());

            // Auto-connect WebSocket if paired
            if config.paired {
                if let Some(token) = &config.agent_token {
                    if let Some(store_id) = config.store_id {
                        let state_clone = state.clone();
                        let tokenClone = token.clone();
                        tokio::spawn(async move {
                            let app = state_clone.lock().await;
                            let _ = app.ws_manager.lock().await.connect(&tokenClone, store_id).await;
                        });
                    }
                }
            }

            // Setup system tray
            let _tray = TrayIconBuilder::new()
                .tooltip("Ordoo Agent")
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Minimize to tray instead of closing
                window.hide().ok();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_dashboard,
            commands::start_pairing,
            commands::get_queue_jobs,
            commands::get_logs,
            commands::get_config,
            commands::reconnect,
            commands::unpair,
            commands::add_printer,
            commands::remove_printer,
            commands::test_printer,
            commands::get_printers,
            commands::discover_printers,
            commands::get_server_printers,
            commands::test_server_printer,
            commands::unpair_with_server,
            commands::check_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
