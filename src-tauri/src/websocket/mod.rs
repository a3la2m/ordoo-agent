use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsEvent {
    pub event: String,
    pub channel: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WsStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

impl WsStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Disconnected => "غير متصل",
            Self::Connecting => "جاري الاتصال...",
            Self::Connected => "متصل",
            Self::Reconnecting => "إعادة اتصال...",
        }
    }
}

pub struct WsManager {
    pub status: Arc<Mutex<WsStatus>>,
    pub ws_url: String,
    pub event_sender: Option<mpsc::UnboundedSender<WsEvent>>,
}

impl WsManager {
    pub fn new(ws_url: String) -> Self {
        Self {
            status: Arc::new(Mutex::new(WsStatus::Disconnected)),
            ws_url,
            event_sender: None,
        }
    }

    pub async fn connect(&mut self, token: &str, store_id: i64) -> Result<(), String> {
        let channel = format!("printers.store.{}", store_id);
        let url = format!("{}/app/{}?token={}", self.ws_url, channel, token);

        *self.status.lock().await = WsStatus::Connecting;
        log::info!("Connecting WebSocket to {}", url);

        match connect_async(&url).await {
            Ok((ws_stream, _)) => {
                log::info!("WebSocket connected!");
                *self.status.lock().await = WsStatus::Connected;

                let (mut write, mut read) = ws_stream.split();
                let (tx, _rx) = mpsc::unbounded_channel::<WsEvent>();
                self.event_sender = Some(tx.clone());

                // Subscribe to channel
                let subscribe_msg = serde_json::json!({
                    "event": "pusher:subscribe",
                    "data": { "channel": channel }
                });
                let _ = write.send(Message::Text(subscribe_msg.to_string())).await;

                // Spawn reader task
                tokio::spawn(async move {
                    while let Some(msg) = read.next().await {
                        match msg {
                            Ok(Message::Text(text)) => {
                                if let Ok(event) = serde_json::from_str::<WsEvent>(&text) {
                                    if event.event == "print.job.assigned" || event.event == "print.job.status.changed" {
                                        let _ = tx.send(event);
                                    }
                                }
                            }
                            Ok(Message::Ping(_)) => {
                                let _ = write.send(Message::Pong(vec![])).await;
                            }
                            Ok(Message::Close(_)) => {
                                log::warn!("WebSocket closed by server");
                                break;
                            }
                            Err(e) => {
                                log::error!("WebSocket error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }
                    log::info!("WebSocket reader stopped");
                });

                Ok(())
            }
            Err(e) => {
                log::error!("WebSocket connection failed: {}", e);
                *self.status.lock().await = WsStatus::Disconnected;
                Err(format!("WebSocket connection failed: {}", e))
            }
        }
    }

    pub async fn disconnect(&mut self) {
        self.event_sender = None;
        *self.status.lock().await = WsStatus::Disconnected;
    }

    pub async fn get_status(&self) -> WsStatus {
        self.status.lock().await.clone()
    }
}
