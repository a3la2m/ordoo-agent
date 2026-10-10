use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

/// رسالة مميّزة: السيرفر رفض التوكن (401) — المحطة حُذفت أو فُكّ ربطها
pub const AUTH_REVOKED: &str = "AUTH_REVOKED";

#[derive(Debug, Clone)]
pub struct ApiClient {
    pub server_url: String,
    pub client: reqwest::Client,
    /// unix seconds لآخر استجابة ناجحة من السيرفر — مصدر «حالة الاتصال» في اللوحة
    /// (WS لا يمكن أن يتصل في بيئة التطوير، فالإشارة الحقيقية هي نجاح HTTP)
    pub last_ok: Arc<AtomicI64>,
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
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

/// استجابة النبض — تُستخدم لتعبئة اسم المحطة على التطاقم المرتبطة مسبقًا
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeartbeatInfo {
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub server_time: Option<String>,
    #[serde(default)]
    pub station_name: Option<String>,
    #[serde(default)]
    pub store_name: Option<String>,
}

impl ApiClient {
    pub fn new(server_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self { server_url, client, last_ok: Arc::new(AtomicI64::new(0)) }
    }

    /// علّم آخر تواصل ناجح مع السيرفر (يُستدعى بعد وصول استجابة صالحة)
    pub fn mark_ok(&self) {
        self.last_ok.store(unix_now(), Ordering::Relaxed);
    }

    /// متصل الآن؟ لو نجح أي نداء خلال نافذة الجَذْب (النبض × 3، وبحد أدنى 60 ثانية)
    pub fn http_connected(&self) -> bool {
        let last = self.last_ok.load(Ordering::Relaxed);
        if last <= 0 { return false; }
        let window = 60_i64.max(self.heartbeat_window_secs());
        unix_now() - last <= window
    }

    fn heartbeat_window_secs(&self) -> i64 {
        // القيمة الافتراضية30 ثانية — تُقرأ من الاستجابة لاحقًا لو زادت
        30 * 3
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
    pub async fn heartbeat(&self, token: &str) -> Result<HeartbeatInfo, String> {
        let resp = self.client
            .post(&self.agent_url("/heartbeat"))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        // 401 → المحطة لم تعد موجودة على السيرفر (حُذفت من لوحة التاجر)
        if resp.status().as_u16() == 401 {
            return Err(AUTH_REVOKED.to_string());
        }
        self.mark_ok();
        // فكّ غير حرج: لو شكل الاستجابة تغيّر لا نُفشل النبض
        Ok(resp.json::<HeartbeatInfo>().await.unwrap_or_default())
    }

    /// Fetch pending jobs from server (server shape: id/print_section_id/payload object)
    pub async fn fetch_pending_jobs(&self, token: &str) -> Result<Vec<ServerPrintJob>, String> {
        let resp = self.client
            .get(&self.agent_url("/jobs/pending"))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status().as_u16() == 401 {
            return Err(AUTH_REVOKED.to_string());
        }
        self.mark_ok();

        #[derive(Deserialize)]
        struct JobsResponse {
            success: bool,
            jobs: Vec<ServerPrintJob>,
        }

        let data = resp.json::<JobsResponse>().await.map_err(|e| e.to_string())?;
        if !data.success {
            return Err("استجابة غير متوقعة من السيرفر عند جلب الوظائف".to_string());
        }
        Ok(data.jobs)
    }

    /// تسجيل الدخول من الوكيل: يعيد رابطًا مؤقتًا (#t=) لفتحه في متصفح هذا الجهاز
    pub async fn login(&self, token: &str, email: &str, password: &str) -> Result<LoginResponse, String> {
        let body = serde_json::json!({ "email": email, "password": password });

        let resp = self.client
            .post(&self.agent_url("/login"))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;

        // فك الترميز بصلابة: أي شكل غير متوقع → رسالة عربية صريحة بدل خطأ تقني خام
        let data = serde_json::from_str::<LoginResponse>(&text).unwrap_or_else(|_| LoginResponse {
            success: false,
            message: Some(match status.as_u16() {
                401 => "رفض السيرفر: توكن المحطة غير صالح — أعد الربط".to_string(),
                429 => "محاولات كثيرة خلال دقيقة — انتظر ثم أعد المحاولة".to_string(),
                _ => format!("استجابة غير متوقعة من السيرفر (رمز {})", status.as_u16()),
            }),
            url: None,
            station: None,
            user: None,
            expires_in: None,
        });

        if !status.is_success() || !data.success {
            return Err(data.message
                .unwrap_or_else(|| "بيانات الدخول غير صحيحة أو لا تملك صلاحية في متجر هذه المحطة".to_string()));
        }
        Ok(data)
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
        self.mark_ok();
        Ok(())
    }

    /// Fetch printers from server
    pub async fn fetch_printers(&self, token: &str) -> Result<Vec<ServerPrinterInfo>, String> {
        let resp = self.client
            .get(&self.agent_url("/printers"))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        #[derive(Deserialize)]
        struct PrintersResponse {
            success: bool,
            printers: Vec<ServerPrinterInfo>,
        }

        let data = resp.json::<PrintersResponse>().await.map_err(|e| e.to_string())?;
        if !data.success {
            return Err("استجابة غير متوقعة من السيرفر عند جلب الطابعات".to_string());
        }
        Ok(data.printers)
    }

    /// Test printer connection on server
    pub async fn test_printer(&self, token: &str, printer_id: i64) -> Result<PrinterTestResult, String> {
        let resp = self.client
            .post(&self.agent_url(&format!("/printers/{}/test", printer_id)))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let data = resp.json::<PrinterTestResult>().await.map_err(|e| e.to_string())?;
        Ok(data)
    }

    /// Unpair from server
    pub async fn unpair_from_server(&self, token: &str) -> Result<(), String> {
        let _resp = self.client
            .post(&self.agent_url("/unpair"))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Check connection (ping)
    pub async fn check_connection(&self, token: &str) -> Result<bool, String> {
        self.ping(token).await?;
        Ok(true)
    }

    /// Check for app updates
    pub async fn check_version(&self) -> Result<VersionCheck, String> {
        let url = format!("{}/agent-version.json", self.server_url);
        let resp = self.client.get(&url).send().await.map_err(|e| e.to_string())?;
        let data = resp.json::<VersionCheck>().await.map_err(|e| e.to_string())?;
        Ok(data)
    }

    // --- جلسة المستخدم على الجهاز (اسم المستخدم في البرنامج) ---

    /// قراءة حالة الجلسة — GET /session (دورة المزامنة، قراءة فقط)
    pub async fn get_session(&self, token: &str) -> Result<SessionState, String> {
        let resp = self.client
            .get(&self.agent_url("/session"))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status().as_u16() == 401 {
            return Err(AUTH_REVOKED.to_string());
        }
        self.mark_ok();

        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;

        serde_json::from_str::<SessionState>(&text).map_err(|_| match status.as_u16() {
            401 => "توكن المحطة غير صالح — أعد الربط".to_string(),
            _ => format!("استجابة غير متوقعة من السيرفر (رمز {})", status.as_u16()),
        })
    }

    /// دخول من الويب بتوكن لمرة واحدة — POST /session/login
    pub async fn session_login(&self, token: &str, one_time_token: &str) -> Result<SessionLoginResponse, String> {
        let body = serde_json::json!({ "token": one_time_token });

        let resp = self.client
            .post(&self.agent_url("/session/login"))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;

        let data = serde_json::from_str::<SessionLoginResponse>(&text).unwrap_or_else(|_| SessionLoginResponse {
            success: false,
            message: Some(match status.as_u16() {
                401 => "توكن المحطة غير صالح — أعد الربط".to_string(),
                429 => "محاولات كثيرة خلال دقيقة — انتظر ثم أعد المحاولة".to_string(),
                _ => format!("استجابة غير متوقعة من السيرفر (رمز {})", status.as_u16()),
            }),
            user: None,
            user_id: None,
        });

        if !status.is_success() || !data.success {
            return Err(data.message.unwrap_or_else(|| "فشل تسجيل الدخول في البرنامج".to_string()));
        }
        Ok(data)
    }

    /// خروج من البرنامج — POST /session/logout (يلغي جلسة الويب المرتبطة أيضًا على السيرفر)
    pub async fn session_logout(&self, token: &str) -> Result<String, String> {
        let resp = self.client
            .post(&self.agent_url("/session/logout"))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;

        #[derive(Deserialize)]
        struct LogoutResponse {
            success: bool,
            message: Option<String>,
        }

        let data = serde_json::from_str::<LogoutResponse>(&text).unwrap_or_else(|_| LogoutResponse {
            success: false,
            message: Some(match status.as_u16() {
                401 => "توكن المحطة غير صالح — أعد الربط".to_string(),
                _ => format!("استجابة غير متوقعة من السيرفر (رمز {})", status.as_u16()),
            }),
        });

        if !status.is_success() || !data.success {
            return Err(data.message.unwrap_or_else(|| "فشل تسجيل الخروج".to_string()));
        }
        Ok(data.message.unwrap_or_else(|| "تم تسجيل الخروج من البرنامج والويب".to_string()))
    }

    // --- إعداد الطباعة: تعريف الطابعات من البرنامج (USB أو شبكة فقط) ---

    /// جلب إعداد الطباعة — GET /setup (طابعة الفاتورة + كل قسم وطابعته)
    pub async fn get_print_setup(&self, token: &str) -> Result<PrintSetup, String> {
        let resp = self.client
            .get(&self.agent_url("/setup"))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status().as_u16() == 401 {
            return Err(AUTH_REVOKED.to_string());
        }

        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;
        serde_json::from_str::<PrintSetup>(&text).map_err(|_| {
            format!("استجابة غير متوقعة من السيرفر (رمز {})", status.as_u16())
        })
    }

    /// حفظ طابعة الفاتورة — POST /setup (يُعيد الإعداد المحدَّث)
    pub async fn save_receipt_printer(
        &self,
        token: &str,
        printer: Option<&PrinterConfig>,
    ) -> Result<PrintSetup, String> {
        let body = serde_json::json!({ "receipt_printer": printer });
        self.post_setup(token, body).await
    }

    /// حفظ طابعة قسم — POST /setup (يُعيد الإعداد المحدَّث)
    pub async fn save_section_printer(
        &self,
        token: &str,
        section_id: i64,
        printer: &PrinterConfig,
    ) -> Result<PrintSetup, String> {
        let entry = serde_json::json!({
            "section_id": section_id,
            "name": printer.name,
            "connection_type": printer.connection_type,
            "ip_address": printer.ip_address,
            "port": printer.port,
            "paper_width": printer.paper_width.unwrap_or(80),
            "max_attempts": printer.max_attempts.unwrap_or(3),
            "device_path": printer.device_path,
        });
        let body = serde_json::json!({ "section_printers": [entry] });
        self.post_setup(token, body).await
    }

    async fn post_setup(&self, token: &str, body: serde_json::Value) -> Result<PrintSetup, String> {
        let resp = self.client
            .post(&self.agent_url("/setup"))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status().as_u16() == 401 {
            return Err(AUTH_REVOKED.to_string());
        }

        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;

        if !status.is_success() {
            #[derive(Deserialize)]
            struct ErrBody {
                message: Option<String>,
            }
            let msg = serde_json::from_str::<ErrBody>(&text)
                .ok()
                .and_then(|b| b.message)
                .unwrap_or_else(|| format!("رفض السيرفر الحفظ (رمز {})", status.as_u16()));
            return Err(msg);
        }

        serde_json::from_str::<PrintSetup>(&text).map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerPrinterInfo {
    pub id: i64,
    pub name: String,
    pub connection_type: String,
    pub ip_address: Option<String>,
    pub port: Option<u16>,
    pub section_name: Option<String>,
    /// هل هذه الطابعة مستضافة على هذا الجهاز؟ (يرسلها السيرفر؛ غيابها = نسخة أقدم)
    #[serde(default)]
    pub hosted: Option<bool>,
    /// هل هي طابعة فاتورة هذه المحطة؟ (تعريف واضح في الواجهة)
    #[serde(default)]
    pub is_receipt: Option<bool>,
    /// عرض الورق (58/80) من إعدادات السيرفر
    #[serde(default)]
    pub paper_width: Option<u32>,
    /// مسار جهاز USB على هذه المحطة (من إعدادات السيرفر)
    #[serde(default)]
    pub device_path: Option<String>,
}

/// إعداد طابعة كما يُعرَّف من هذا البرنامج — USB أو شبكة فقط (لا بلوتوث)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterConfig {
    /// معرّفها على السيرفر (غائب قبل الحفظ الأول)
    #[serde(default)]
    pub id: Option<i64>,
    pub name: String,
    /// "usb" | "network"
    pub connection_type: String,
    #[serde(default)]
    pub ip_address: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub paper_width: Option<u32>,
    #[serde(default)]
    pub max_attempts: Option<u32>,
    #[serde(default)]
    pub device_path: Option<String>,
}

/// قسم طباعة من السيرفر وطابعته الحالية (إن وُجدت)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionInfo {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default)]
    pub printer: Option<PrinterConfig>,
}

/// استجابة GET/POST /setup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintSetup {
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub receipt_printer: Option<PrinterConfig>,
    #[serde(default)]
    pub sections: Vec<SectionInfo>,
}

/// وظيفة طباعة كما يرسلها السيرفر (حقول مطابقة لمسلّم Laravel)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerPrintJob {
    pub id: i64,
    pub store_id: i64,
    pub printer_id: Option<i64>,
    pub print_section_id: Option<i64>,
    pub job_type: Option<String>,
    pub status: Option<String>,
    /// كائن JSON (يُخزَّن نصيًا في الطابور المحلي)
    pub payload: serde_json::Value,
    pub attempts: Option<u32>,
    pub last_error: Option<String>,
    /// عدد محاولات الطباعة المعرَّف لطابعة هذه الوظيفة (من Ordoo Agent)
    #[serde(default)]
    pub max_attempts: Option<u32>,
}

/// استجابة نقطة دخول الوكيل — الرابط يحمل الرسغ الخام في الـfragment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub success: bool,
    pub message: Option<String>,
    pub url: Option<String>,
    pub station: Option<String>,
    pub user: Option<String>,
    pub expires_in: Option<u64>,
}

/// حالة جلسة المستخدم على هذا الجهاز (GET /session)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub logged_in: bool,
    pub user: Option<String>,
    pub user_id: Option<i64>,
    pub login_method: Option<String>,
    pub since: Option<String>,
}

/// استجابة دخول الجلسة من الويب (POST /session/login)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLoginResponse {
    pub success: bool,
    pub message: Option<String>,
    pub user: Option<String>,
    pub user_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterTestResult {
    pub success: bool,
    pub connected: bool,
    pub response_time_ms: Option<u64>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionCheck {
    pub version: String,
    pub download_url: Option<String>,
    pub changelog: Option<String>,
}
