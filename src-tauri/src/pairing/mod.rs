use serde::{Deserialize, Serialize};

/// Pairing request sent to Laravel (simplified token-based)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingRequest {
    pub token: String,
    pub name: Option<String>,
    pub machine_id: Option<String>,
}

/// Pairing response from Laravel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingResponse {
    pub success: bool,
    pub message: Option<String>,
    pub agent_id: Option<i64>,
    pub store_id: Option<i64>,
    pub token: Option<String>,
    pub station_name: Option<String>,
}

/// Pairing status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PairingStatus {
    Unpaired,
    Pending,
    Paired,
    Expired,
    Failed,
}

impl PairingStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Unpaired => "غير مربوط",
            Self::Pending => "قيد الانتظار",
            Self::Paired => "مربوط",
            Self::Expired => "منتهي الصلاحية",
            Self::Failed => "فشل",
        }
    }
}
