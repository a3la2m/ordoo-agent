use serde::{Deserialize, Serialize};

/// Printer connection types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectionType {
    Usb,
    Network,
    Bluetooth,
    WindowsDriver,
}

impl ConnectionType {
    pub fn label(&self) -> &str {
        match self {
            Self::Usb => "USB",
            Self::Network => "شبكة",
            Self::Bluetooth => "بلوتوث",
            Self::WindowsDriver => "طابعة Windows",
        }
    }
}

/// Printer representation in the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Printer {
    pub id: i64,
    pub name: String,
    pub connection_type: ConnectionType,
    pub ip_address: Option<String>,
    pub port: Option<u16>,
    pub paper_width: Option<u32>,
    pub is_online: bool,
    pub last_test_at: Option<String>,
    pub last_test_success: Option<bool>,
}

/// Printer status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterStatus {
    pub online: bool,
    pub error: Option<String>,
    pub paper_out: bool,
    pub cover_open: bool,
}

impl Printer {
    pub fn new(
        id: i64,
        name: String,
        connection_type: ConnectionType,
        ip_address: Option<String>,
        port: Option<u16>,
    ) -> Self {
        Self {
            id,
            name,
            connection_type,
            ip_address,
            port,
            paper_width: Some(80),
            is_online: false,
            last_test_at: None,
            last_test_success: None,
        }
    }

    /// Check if printer is reachable (TCP ping for network printers)
    pub async fn check_connection(&self) -> PrinterStatus {
        match self.connection_type {
            ConnectionType::Network => {
                if let (Some(ip), Some(port)) = (&self.ip_address, self.port) {
                    let addr = format!("{}:{}", ip, port);
                    match tokio::net::TcpStream::connect(&addr).await {
                        Ok(_) => PrinterStatus {
                            online: true,
                            error: None,
                            paper_out: false,
                            cover_open: false,
                        },
                        Err(e) => PrinterStatus {
                            online: false,
                            error: Some(e.to_string()),
                            paper_out: false,
                            cover_open: false,
                        },
                    }
                } else {
                    PrinterStatus {
                        online: false,
                        error: Some("Missing IP or port".to_string()),
                        paper_out: false,
                        cover_open: false,
                    }
                }
            }
            _ => PrinterStatus {
                online: false,
                error: Some("Connection type not yet supported for auto-detection".to_string()),
                paper_out: false,
                cover_open: false,
            },
        }
    }

    /// Send test print (ESC/POS hello)
    pub async fn test_print(&self) -> Result<(), String> {
        match self.connection_type {
            ConnectionType::Network => {
                if let (Some(ip), Some(port)) = (&self.ip_address, self.port) {
                    let addr = format!("{}:{}", ip, port);
                    use tokio::io::AsyncWriteExt;
                    let mut stream = tokio::net::TcpStream::connect(&addr)
                        .await
                        .map_err(|e| e.to_string())?;

                    // ESC/POS: Initialize, set bold, print test, cut
                    let test_data = b"\x1B\x40"; // Initialize
                    stream.write_all(test_data).await.map_err(|e| e.to_string())?;
                    let title = format!("\x1B\x61\x01  ORDOO AGENT TEST  \x1B\x61\x00\n");
                    stream.write_all(title.as_bytes()).await.map_err(|e| e.to_string())?;
                    let time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                    stream.write_all(format!("{}\n\n\n", time).as_bytes()).await.map_err(|e| e.to_string())?;
                    stream.write_all(b"\x1D\x56\x00").await.map_err(|e| e.to_string())?; // Cut

                    Ok(())
                } else {
                    Err("Missing IP or port".to_string())
                }
            }
            _ => Err("Test print only supported for network printers in v1".to_string()),
        }
    }
}

pub mod engine;
pub mod manager;
pub use manager::PrinterManager;
