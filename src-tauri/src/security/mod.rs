use std::fs;
use std::path::PathBuf;

pub struct SecureStore {
    base_path: PathBuf,
}

impl SecureStore {
    pub fn new(app_name: &str) -> Self {
        let base_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(app_name);

        if !base_path.exists() {
            let _ = fs::create_dir_all(&base_path);
        }

        Self { base_path }
    }

    fn token_path(&self) -> PathBuf {
        self.base_path.join("agent_token.dat")
    }

    fn identity_path(&self) -> PathBuf {
        self.base_path.join("identity.json")
    }

    fn config_path(&self) -> PathBuf {
        self.base_path.join("config.json")
    }

    // --- Token ---
    pub fn save_token(&self, token: &str) -> Result<(), String> {
        fs::write(self.token_path(), token).map_err(|e| e.to_string())
    }

    pub fn load_token(&self) -> Option<String> {
        fs::read_to_string(self.token_path()).ok()
    }

    pub fn clear_token(&self) -> Result<(), String> {
        let path = self.token_path();
        if path.exists() {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    // --- Identity ---
    pub fn save_identity(&self, identity: &super::identity::PersistedIdentity) -> Result<(), String> {
        let json = serde_json::to_string_pretty(identity).map_err(|e| e.to_string())?;
        fs::write(self.identity_path(), json).map_err(|e| e.to_string())
    }

    pub fn load_identity(&self) -> Option<super::identity::PersistedIdentity> {
        let data = fs::read_to_string(self.identity_path()).ok()?;
        serde_json::from_str(&data).ok()
    }

    // --- Config ---
    pub fn save_config(&self, config: &super::AgentConfig) -> Result<(), String> {
        let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
        fs::write(self.config_path(), json).map_err(|e| e.to_string())
    }

    pub fn load_config(&self) -> Option<super::AgentConfig> {
        let data = fs::read_to_string(self.config_path()).ok()?;
        serde_json::from_str(&data).ok()
    }
}
