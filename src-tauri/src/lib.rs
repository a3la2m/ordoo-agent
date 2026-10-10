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
use tauri::Emitter;
use tauri_plugin_deep_link::DeepLinkExt;
use tauri::tray::{TrayIconBuilder, MouseButton, MouseButtonState, TrayIconEvent};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::image::Image;
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
    /// اسم المحطة كما هي مسجّلية على السيرفر (من PairingResponse/heartbeat)
    #[serde(default)]
    pub station_name: Option<String>,
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
    /// اسم المستخدم المسجّل دخوله في البرنامج (جلسة الجهاز)
    #[serde(default)]
    pub session_user: Option<String>,
    #[serde(default)]
    pub session_user_id: Option<i64>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            store_id: None,
            store_name: None,
            station_name: None,
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
            session_user: None,
            session_user_id: None,
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

/// معالج الروابط العميقة:
/// `ordoo-agent://pair?server=...&token=...` — ربط المحطة
/// `ordoo-agent://login?server=...&token=...` — دخول المستخدم إلى البرنامج (جلسة)
fn dispatch_deep_link(app: tauri::AppHandle, state: Arc<Mutex<AppState>>, url: &url::Url) {
    if url.scheme() != "ordoo-agent" {
        return;
    }
    let host = url.host_str().unwrap_or_default().to_string();
    if host != "pair" && host != "login" {
        return;
    }

    let params: std::collections::HashMap<String, String> = url.query_pairs().into_owned().collect();
    let server = params.get("server").cloned();
    let token = params.get("token").cloned();
    match host.as_str() {
        // الدخول يستهلك توكنًا لمرة واحدة — رابط بلا توكن مرفوض
        "login" if token.is_none() => return,
        "pair" if server.is_none() && token.is_none() => return,
        _ => {}
    }

    // أظهر النافذة ليستقبل المستخدم نتيجة الرابط
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }

    tauri::async_runtime::spawn(async move {
        let result = if host == "login" {
            commands::deep_link_session_login(state, server, token).await
        } else {
            commands::deep_link_connect(state, server, token).await
        };
        let payload = match &result {
            Ok(message) => serde_json::json!({"success": true, "message": message}),
            Err(message) => serde_json::json!({"success": false, "message": message}),
        };
        let _ = app.emit("agent://deep-link-result", payload);
    });
}

/// إنشاء/تحديث إدخال autostart في Linux — يعمل البرنامج مع بدء الجهاز تلقائيًا.
/// يُعاد الكتابة مع كل تشغيل ليظل المسار صحيحًا بعد تحديث الإصدار.
fn ensure_autostart() {
    let Ok(home) = std::env::var("HOME") else { return };

    // AppImage: مسار الملف الحقيقي في APPIMAGE (current_exe داخل نقطة تركيب مؤقتة)
    let exec = std::env::var("APPIMAGE")
        .ok()
        .filter(|p| !p.is_empty())
        .or_else(|| std::env::current_exe().ok().map(|p| p.display().to_string()));
    let Some(exec) = exec else { return };

    let dir = std::path::Path::new(&home).join(".config/autostart");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::warn!("autostart dir failed: {e}");
        return;
    }

    let file = dir.join("ordoo-agent.desktop");
    let content = format!(
        "[Desktop Entry]\nType=Application\nName=Ordoo Agent\nComment=Ordoo print agent\nExec=\"{exec}\"\nTerminal=false\nX-GNOME-Autostart-enabled=true\nNoDisplay=true\n"
    );
    match std::fs::write(&file, &content) {
        Ok(_) => log::info!("autostart entry ready: {}", file.display()),
        Err(e) => log::warn!("autostart entry failed: {e}"),
    }
}

/// السيرفر رفض التوكن (401): المحطة حُذفت من لوحة التاجر →
/// فك ارتباط تلقائي + تسجيل خروج المستخدم من البرنامج + إشعار الواجهة
async fn handle_auth_revoked(
    state: &Arc<tokio::sync::Mutex<crate::AppState>>,
    app: &tauri::AppHandle,
) {
    // تفادي التكرار: إن كان التوكن ممسوحًا بالفعل فلا شيء
    {
        let st = state.lock().await;
        let cfg = st.config.lock().await;
        if cfg.agent_token.is_none() {
            return;
        }
    }

    let st = state.lock().await;
    if let Err(e) = crate::commands::local_unpair(&st).await {
        log::error!("auto-unpair after 401 failed: {e}");
    }
    {
        let db = st.db.lock().await;
        let _ = logs::AgentLogger::warn(
            &db,
            "pairing",
            "السيرفر رفض التوكن (401) — المحطة حُذفت: فك ارتباط تلقائي وتسجيل خروج المستخدم",
        );
    }
    let _ = app.emit(
        "agent://unpaired",
        serde_json::json!({
            "message": "حُذفت المحطة من السيرفر — تم تسجيل الخروج وفك الارتباط",
        }),
    );
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // If another instance is running, focus existing window
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
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

            // تحديث رقم الإصدار المحفوظ (قد يكون قديمًا من نسخة سابقة)
            let current_version = env!("CARGO_PKG_VERSION");
            if config.version != current_version {
                config.version = current_version.to_string();
                let _ = security_store.save_config(&config);
            }

            // Try to load saved token
            if let Some(token) = security_store.load_token() {
                config.agent_token = Some(token);
                config.paired = true;
            }

            // Log startup
            {
                let _ = logs::AgentLogger::info(&conn, "system", &format!("Ordoo Agent v{} started", config.version));
            }

            // التشغيل التلقائي مع بدء الجهاز (Linux) — إدخال autostart يُنشأ مع أول تشغيل
            ensure_autostart();

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

            // الروابط العميقة من الويب: ordoo-agent://pair?server=...&token=...
            {
                let state_for_open = state.clone();
                let app_for_open = app.handle().clone();
                app.deep_link().on_open_url(move |event| {
                    for url in event.urls() {
                        dispatch_deep_link(app_for_open.clone(), state_for_open.clone(), &url);
                    }
                });

                // الإقلاع البارد: الرابط الذي فُتح به البرنامج نفسه
                if let Ok(Some(urls)) = app.deep_link().get_current() {
                    for url in &urls {
                        dispatch_deep_link(app.handle().clone(), state.clone(), url);
                    }
                }

                // ضمان تسجيل المخطط في نظام التشغيل (خصوصًا AppImage)
                if let Err(e) = app.deep_link().register_all() {
                    log::warn!("deep-link register failed: {e}");
                }
            }

            // حلقة المزامنة الدورية: نبض + جلب وظائف الطباعة وتنفيذها + مزامنة الطابعات
            {
                let state_clone = state.clone();
                let sync_app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    const TICK_SECS: u64 = 5;
                    const PRINTERS_EVERY_TICKS: u64 = 12; // ≈ دقيقة
                    const SESSION_EVERY_TICKS: u64 = 2; // ≈10 ثوانٍ — مزامنة اسم المستخدم
                    let mut tick: u64 = 0;

                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(TICK_SECS)).await;
                        tick += 1;

                        // نسخ المراجع ثم إفلات القفل قبل أي انتظار شبكة
                        let (api_client, db, token, heartbeat_every) = {
                            let app = state_clone.lock().await;
                            let (token, hb) = {
                                let cfg = app.config.lock().await;
                                (cfg.agent_token.clone(), cfg.heartbeat_interval_secs)
                            };
                            (app.api_client.clone(), app.db.clone(), token, hb)
                        };

                        // غير مربوط → لا شيء هذا الدور
                        let Some(token) = token else { continue };

                        let hb_ticks = (heartbeat_every / TICK_SECS).max(1);
                        if tick % hb_ticks == 0 {
                            match api_client.heartbeat(&token).await {
                                Err(e) if e == crate::api::AUTH_REVOKED => {
                                    handle_auth_revoked(&state_clone, &sync_app_handle).await;
                                    continue;
                                }
                                Err(e) => log::debug!("heartbeat failed: {}", e),
                                Ok(hb) => {
                                    // تعبئة اسم المحطة/المتجر من السيرفر (خصوصًا التطاقم القديمة)
                                    let changed = {
                                        let app = state_clone.lock().await;
                                        let mut cfg = app.config.lock().await;
                                        let mut dirty = false;
                                        if let Some(name) = hb.station_name.filter(|s| !s.trim().is_empty()) {
                                            if cfg.station_name.as_deref() != Some(name.as_str()) {
                                                cfg.station_name = Some(name);
                                                dirty = true;
                                            }
                                        }
                                        if let Some(name) = hb.store_name.filter(|s| !s.trim().is_empty()) {
                                            if cfg.store_name.as_deref() != Some(name.as_str()) {
                                                cfg.store_name = Some(name);
                                                dirty = true;
                                            }
                                        }
                                        if dirty {
                                            let _ = app.security_store.save_config(&cfg);
                                        }
                                        dirty
                                    };
                                    if changed {
                                        let _ = sync_app_handle.emit("agent://config-changed", serde_json::json!({}));
                                    }
                                }
                            }
                        }

                        // مزامنة جلسة المستخدم: قراءة فقط + تحديث عند التغيير فقط
                        if tick % SESSION_EVERY_TICKS == 0 {
                            match api_client.get_session(&token).await {
                                Ok(session_state) => {
                                    let new_user = if session_state.logged_in { session_state.user.clone() } else { None };
                                    let new_id = if session_state.logged_in { session_state.user_id } else { None };
                                    let (changed, display) = {
                                        let app = state_clone.lock().await;
                                        let mut cfg = app.config.lock().await;
                                        if cfg.session_user == new_user && cfg.session_user_id == new_id {
                                            (false, None)
                                        } else {
                                            cfg.session_user = new_user.clone();
                                            cfg.session_user_id = new_id;
                                            let _ = app.security_store.save_config(&cfg);
                                            (true, new_user.clone())
                                        }
                                    };
                                    if changed {
                                        let message = match &display {
                                            Some(name) => format!("تسجيل دخول في البرنامج: {}", name),
                                            None => "تم تسجيل الخروج من البرنامج".to_string(),
                                        };
                                        {
                                            let app = state_clone.lock().await;
                                            let db = app.db.lock().await;
                                            let _ = logs::AgentLogger::info(&db, "session", &message);
                                        }
                                        let _ = sync_app_handle.emit("agent://session-changed", serde_json::json!({
                                            "message": message,
                                            "user": display,
                                        }));
                                    }
                                }
                                Err(e) if e == crate::api::AUTH_REVOKED => {
                                    handle_auth_revoked(&state_clone, &sync_app_handle).await;
                                    continue;
                                }
                                Err(e) => log::debug!("session sync: {}", e),
                            }
                        }

                        if tick % PRINTERS_EVERY_TICKS == 0 {
                            let engine = crate::sync::SyncEngine::new(api_client.clone(), db.clone());
                            match engine.sync_printers(&token).await {
                                Ok(n) if n > 0 => log::info!("synced {} server printers locally", n),
                                Ok(_) => {}
                                Err(e) => log::debug!("printers sync: {}", e),
                            }
                        }

                        let engine = crate::sync::SyncEngine::new(api_client, db);

                        // جلب الوظائف الجديدة (idempotent) — فشل الشبكة لا يمنع معالجة الطابور المحلي
                        match engine.sync_pending_jobs(&token).await {
                            Ok(n) if n > 0 => log::info!("enqueued {} new print jobs", n),
                            Ok(_) => {}
                            Err(e) if e == crate::api::AUTH_REVOKED => {
                                handle_auth_revoked(&state_clone, &sync_app_handle).await;
                                continue;
                            }
                            Err(e) => log::debug!("jobs sync: {}", e),
                        }

                        // تنفيذ الطابور حتى الاستنفاد (بحد أقصى10 وظائف لكل دورة)
                        for _ in 0..10 {
                            match engine.process_next_job(&token).await {
                                Ok(true) => continue,
                                Ok(false) => break,
                                Err(e) => {
                                    log::error!("job processing error: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                });
            }

            // Auto-connect WebSocket if paired (use tauri async runtime, not tokio::spawn)
            if config.paired {
                if let Some(token) = &config.agent_token {
                    if let Some(store_id) = config.store_id {
                        let state_clone = state.clone();
                        let token_clone = token.clone();
                        tauri::async_runtime::spawn(async move {
                            let app = state_clone.lock().await;
                            let _ = app.ws_manager.lock().await.connect(&token_clone, store_id).await;
                        });
                    }
                }
            }

            // Setup system tray with icon and menu
            let show_item = MenuItemBuilder::with_id("show", "إظهار النافذة").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "إنهاء").build(app)?;
            let menu = MenuBuilder::new(app).item(&show_item).item(&quit_item).build()?;

            // Try to load icon from the bundled resources
            let app_handle = app.handle().clone();
            let icon_path = app_handle.path().resource_dir()
                .ok()
                .and_then(|p| {
                    let path = p.join("icons/icon.png");
                    if path.exists() { Some(path) } else { None }
                });

            let icon_image = match icon_path {
                Some(p) => {
                    let img = image::open(&p).expect("Failed to load tray icon from path");
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    Image::new_owned(rgba.into_raw(), w, h)
                }
                None => {
                    let img = image::load_from_memory(include_bytes!("../icons/icon.png"))
                        .expect("Failed to load tray icon from bytes");
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    Image::new_owned(rgba.into_raw(), w, h)
                }
            };

            let _tray = TrayIconBuilder::with_id("ordoo-agent-tray")
                .tooltip("Ordoo Agent -(ordoo.shop)")
                .icon(icon_image)
                .menu(&menu)
                .on_menu_event(move |app, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
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
            commands::check_for_updates,
            commands::detect_usb_printers,
            commands::list_usb_nodes,
            commands::set_printer_device_path,
            commands::set_printer_paper_width,
            commands::get_print_setup,
            commands::save_receipt_printer,
            commands::save_section_printer,
            commands::test_printer_config,
            commands::agent_login,
            commands::agent_session_logout,
            commands::open_external,
            commands::set_server_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
