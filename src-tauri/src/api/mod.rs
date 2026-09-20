use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ApiClient {
    pub server_url: String,
    pub client: reqwest::Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: Option<String>,
    pub data: Option<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: i64,
    pub name: String,
    pub store_id: i64,
    pub version: Option<String>,
    pub is_online: bool,
}

impl ApiClient {
    pub fn new(server_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self { server_url, client }
    }

    fn agent_url(&self, path: &str) -> String {
        format!("{}/api/v1/agent{}", self.server_url, path)
    }

    /// Pair with Laravel server (simplified token-based)
    pub async fn pair(
        &self,
        token: &str,
        name: &str,
        machine_id: &str,
    ) -> Result<super::pairing::PairingResponse, String> {
        let body = serde_json::json!({
            "token": token,
            "name": name,
            "machine_id": machine_id,
        });

        let resp = self.client
            .post(&self.agent_url("/pair"))
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        resp.json::<super::pairing::PairingResponse>()
            .await
            .map_err(|e| e.to_string())
    }

    /// Send heartbeat
    pub async fn heartbeat(&self, token: &str) -> Result<(), String> {
        let _resp = self.client
            .post(&self.agent_url("/heartbeat"))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Fetch pending jobs
    pub async fn fetch_pending_jobs(&self, token: &str) -> Result<Vec<super::queue::PrintJob>, String> {
        let resp = self.client
            .get(&self.agent_url("/jobs/pending"))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        #[derive(Deserialize)]
        struct JobsResponse {
            success: bool,
            jobs: Vec<super::queue::PrintJob>,
        }

        let data = resp.json::<JobsResponse>().await.map_err(|e| e.to_string())?;
        Ok(data.jobs)
    }

    /// Update job status on server
    pub async fn update_job_status(
        &self,
        token: &str,
        job_id: i64,
        status: &str,
        error: Option<&str>,
    ) -> Result<(), String> {
        let mut body = serde_json::json!({ "status": status });
        if let Some(err) = error {
            body["error_message"] = serde_json::json!(err);
        }

        let _resp = self.client
            .patch(&self.agent_url(&format!("/jobs/{}/status", job_id)))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Ping server
    pub async fn ping(&self, token: &str) -> Result<(), String> {
        let _resp = self.client
            .get(&self.agent_url("/ping"))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
