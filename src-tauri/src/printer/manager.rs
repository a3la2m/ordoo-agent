use super::{Printer, PrinterStatus};
use std::collections::HashMap;

pub struct PrinterManager {
    printers: HashMap<i64, Printer>,
}

impl PrinterManager {
    pub fn new() -> Self {
        Self {
            printers: HashMap::new(),
        }
    }

    pub fn add_printer(&mut self, printer: Printer) {
        self.printers.insert(printer.id, printer);
    }

    pub fn remove_printer(&mut self, id: i64) -> Option<Printer> {
        self.printers.remove(&id)
    }

    pub fn get_printer(&self, id: i64) -> Option<&Printer> {
        self.printers.get(&id)
    }

    pub fn get_all_printers(&self) -> Vec<&Printer> {
        self.printers.values().collect()
    }

    pub async fn check_all_printers(&mut self) -> Vec<(i64, PrinterStatus)> {
        let mut results = Vec::new();
        for (id, printer) in &self.printers {
            let status = printer.check_connection().await;
            results.push((*id, status));
        }
        results
    }

    pub async fn test_printer(&self, id: i64) -> Result<(), String> {
        if let Some(printer) = self.printers.get(&id) {
            printer.test_print().await
        } else {
            Err("Printer not found".to_string())
        }
    }

    /// Discover network printers by scanning common ports
    pub async fn discover_network_printers(subnet: &str) -> Vec<DiscoveredPrinter> {
        let mut found = Vec::new();

        // Scan port 9100 (standard raw printing port) on the subnet
        for i in 1..=254 {
            let ip = format!("{}.{}", subnet, i);
            let addr = format!("{}:9100", ip);
            let socket_addr: std::net::SocketAddr = addr.parse().unwrap();
            if tokio::time::timeout(
                std::time::Duration::from_millis(200),
                tokio::net::TcpStream::connect(&socket_addr),
            ).await.is_ok() {
                found.push(DiscoveredPrinter {
                    ip,
                    port: 9100,
                    name: "Network Printer".to_string(),
                });
            }
        }

        found
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiscoveredPrinter {
    pub ip: String,
    pub port: u16,
    pub name: String,
}
