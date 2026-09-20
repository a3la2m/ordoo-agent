use serde::{Deserialize, Serialize};

/// Machine-level identity for the agent
/// Not tied to any Windows/Linux user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub machine_id: String,
    pub installation_id: String,
    pub agent_id: Option<i64>,
    pub store_id: Option<i64>,
}

/// Persisted identity data saved to disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedIdentity {
    pub machine_id: String,
    pub installation_id: String,
}

impl AgentIdentity {
    pub fn load_or_create(security_store: &super::security::SecureStore) -> Self {
        // Try to load persisted identity
        if let Some(persisted) = security_store.load_identity() {
            log::info!("Loaded persisted identity: machine={}", &persisted.machine_id[..8]);
            return Self {
                machine_id: persisted.machine_id,
                installation_id: persisted.installation_id,
                agent_id: None,
                store_id: None,
            };
        }

        // Generate new identity
        let machine_id = Self::generate_machine_id();
        let installation_id = Self::generate_installation_id();

        // Persist it
        let persisted = PersistedIdentity {
            machine_id: machine_id.clone(),
            installation_id: installation_id.clone(),
        };
        let _ = security_store.save_identity(&persisted);

        log::info!("Generated new identity: machine={}", &machine_id[..8]);

        Self {
            machine_id,
            installation_id,
            agent_id: None,
            store_id: None,
        }
    }

    /// Generate a stable machine ID based on hardware
    pub fn generate_machine_id() -> String {
        if let Ok(uid) = machine_uid::get() {
            return uid;
        }

        // Fallback: hash of hostname
        use sha2::{Digest, Sha256};
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        let mut hasher = Sha256::new();
        hasher.update(format!("ordoo-agent-{}", hostname));
        hex::encode(hasher.finalize())
    }

    /// Generate a unique installation ID (fresh for each install)
    pub fn generate_installation_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    pub fn is_paired(&self) -> bool {
        self.agent_id.is_some() && self.store_id.is_some()
    }
}
