import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-shell";

interface DashboardData {
  paired: boolean;
  store_name: string | null;
  station_name: string | null;
  server_url: string;
  machine_name: string;
  agent_id: number | null;
  version: string;
  ws_status: string;
  connected: boolean;
  printers: any[];
  queue_stats: { pending: number; printing: number; completed: number; failed: number };
  last_job: any | null;
}

interface UpdateInfo {
  has_update: boolean;
  current_version: string;
  latest_version: string;
  download_url?: string;
  changelog?: string;
}

interface SessionConfig {
  paired: boolean;
  session_user: string | null;
}

export default function Dashboard({ config }: { config: SessionConfig | null }) {
  const [data, setData] = useState<DashboardData | null>(null);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [loginEmail, setLoginEmail] = useState("");
  const [loginPassword, setLoginPassword] = useState("");
  const [loginLoading, setLoginLoading] = useState(false);
  const [loginError, setLoginError] = useState<string | null>(null);
  const [loginNotice, setLoginNotice] = useState<string | null>(null);

  useEffect(() => {
    loadData();
    checkForUpdates();
    const interval = setInterval(loadData, 5000);
    return () => clearInterval(interval);
  }, []);

  async function loadData() {
    try {
      const result = await invoke<DashboardData>("get_dashboard");
      setData(result);
    } catch (e) {
      console.error(e);
    }
  }

  async function checkForUpdates() {
    try {
      const result = await invoke<UpdateInfo>("check_for_updates");
      setUpdateInfo(result);
    } catch (e) {
      console.error("Failed to check for updates:", e);
    }
  }

  async function openWebsite() {
    try {
      await open("https://ordoo.shop");
    } catch (e) {
      console.error("Failed to open website:", e);
      // Fallback
      window.open("https://ordoo.shop", "_blank");
    }
  }

  // تسجيل الدخول من الوكيل: يعيد رابطًا لمرة واحدة يُفتح في متصفح هذا الجهاز
  async function handleAgentLogin() {
    if (!loginEmail.trim() || !loginPassword) {
      setLoginError("أدخل البريد الإلكتروني وكلمة المرور");
      return;
    }
    setLoginLoading(true);
    setLoginError(null);
    setLoginNotice(null);
    try {
      const res = await invoke<{ success: boolean; url?: string; user?: string | null }>("agent_login", {
        email: loginEmail.trim(),
        password: loginPassword,
      });
      if (res.url) {
        await invoke("open_external", { url: res.url });
        setLoginPassword("");
        setLoginNotice(`تم فتح صفحة الدخول${res.user ? ` — ${res.user}` : ""} في متصفح هذا الجهاز`);
      } else {
        setLoginError("لم يُعِد السيرفر رابطًا");
      }
    } catch (e: any) {
      setLoginError(String(e));
    } finally {
      setLoginLoading(false);
    }
  }

  if (!data) return <div style={{ textAlign: "center", padding: 40, color: "#7a8d9e" }}>جاري التحميل...</div>;

  return (
    <div style={{ maxWidth: 700, margin: "0 auto" }}>
      {/* Update Banner */}
      {updateInfo?.has_update && (
        <div style={{
          background: "linear-gradient(135deg, #2563eb, #3b82f6)",
          borderRadius: 12, padding: "14px 20px", marginBottom: 20,
          display: "flex", alignItems: "center", justifyContent: "space-between",
        }}>
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <div style={{
              width: 40, height: 40, borderRadius: "50%",
              background: "rgba(255,255,255,0.2)",
              display: "flex", alignItems: "center", justifyContent: "center",
              fontSize: "1.2rem",
            }}>
              🆕
            </div>
            <div>
              <div style={{ color: "white", fontWeight: 700, fontSize: "0.9rem" }}>
                إصدار جديد متاح: v{updateInfo.latest_version}
              </div>
              {updateInfo.changelog && (
                <div style={{ color: "rgba(255,255,255,0.85)", fontSize: "0.75rem", marginTop: 2 }}>
                  {updateInfo.changelog}
                </div>
              )}
            </div>
          </div>
          <button
            onClick={() => {
              const url = updateInfo.download_url || "https://ordoo.shop/merchant/agent/download";
              open(url).catch(() => window.open(url, "_blank"));
            }}
            style={{
              padding: "8px 16px", borderRadius: 8, border: "none",
              background: "white", color: "#2563eb",
              fontSize: "0.8rem", fontWeight: 700, cursor: "pointer",
              whiteSpace: "nowrap",
            }}
          >
            <i className="fas fa-download ml-1"></i> تحميل
          </button>
        </div>
      )}

      {/* تسجيل الدخول على هذا الجهاز — يظهر فقط ما دام الجهاز مربوطًا ولا جلسة نشطة */}
      {config && config.paired && !config.session_user && (
        <div style={{ background: "white", borderRadius: 16, padding: 20, border: "1px solid #f0f2f5", marginBottom: 20 }}>
          <h3 style={{ margin: "0 0 4px", fontSize: "0.9rem", fontWeight: 700 }}>
            <i className="fas fa-sign-in-alt ml-2" style={{ color: "#2563eb" }}></i>
            تسجيل الدخول على هذا الجهاز
          </h3>
          <p style={{ fontSize: "0.75rem", color: "#7a8d9e", margin: "0 0 12px" }}>
            يفتح متصفح هذا الجهاز على لوحة التاجر مباشرة — الرابط صالح120 ثانية ولمرة واحدة
          </p>

          <div style={{ display: "grid", gap: 8, marginBottom: 10 }}>
            <input
              type="email" value={loginEmail}
              onChange={e => setLoginEmail(e.target.value)}
              placeholder="البريد الإلكتروني"
              dir="ltr"
              style={{
                width: "100%", padding: "10px 14px", border: "2px solid #e5e7eb",
                borderRadius: 10, fontSize: "0.85rem", outline: "none", boxSizing: "border-box",
              }}
            />
            <input
              type="password" value={loginPassword}
              onChange={e => setLoginPassword(e.target.value)}
              onKeyDown={e => { if (e.key === "Enter") handleAgentLogin(); }}
              placeholder="كلمة المرور"
              dir="ltr"
              style={{
                width: "100%", padding: "10px 14px", border: "2px solid #e5e7eb",
                borderRadius: 10, fontSize: "0.85rem", outline: "none", boxSizing: "border-box",
              }}
            />
          </div>

          {loginError && (
            <div style={{ background: "#fef2f2", color: "#dc2626", padding: "8px 12px", borderRadius: 8, fontSize: "0.8rem", marginBottom: 10 }}>
              {loginError}
            </div>
          )}
          {loginNotice && (
            <div style={{ background: "#f0f9ff", color: "#2563eb", padding: "8px 12px", borderRadius: 8, fontSize: "0.8rem", marginBottom: 10 }}>
              {loginNotice}
            </div>
          )}

          <button onClick={handleAgentLogin} disabled={loginLoading} style={{
            width: "100%", padding: "10px",
            background: loginLoading ? "#a0a8b0" : "#2563eb",
            color: "white", borderRadius: 10, border: "none", fontWeight: 700,
            cursor: loginLoading ? "not-allowed" : "pointer",
          }}>
            {loginLoading ? (
              <><i className="fas fa-spinner fa-spin ml-2"></i> جاري الطلب...</>
            ) : (
              <><i className="fas fa-external-link-alt ml-2"></i> فتح صفحة الدخول في المتصفح</>
            )}
          </button>
        </div>
      )}

      {/* Connection Status */}
      <div style={{
        background: data.connected ? "linear-gradient(135deg, #16a34a, #22c55e)" : "linear-gradient(135deg, #d97706, #f59e0b)",
        borderRadius: 16, padding: "20px 24px", marginBottom: 20,
        display: "flex", alignItems: "center", gap: 16,
      }}>
        <div style={{
          width: 48, height: 48, borderRadius: "50%",
          background: "rgba(255,255,255,0.2)",
          display: "flex", alignItems: "center", justifyContent: "center",
          fontSize: "1.5rem",
        }}>
          {data.connected ? "🟢" : "🟡"}
        </div>
        <div>
          <div style={{ color: "white", fontSize: "1.1rem", fontWeight: 800 }}>
            {data.connected ? "متصل" : (data.paired ? "غير متصل" : "غير مربوط")}
          </div>
          <div style={{ color: "rgba(255,255,255,0.85)", fontSize: "0.8rem" }}>
            {data.paired
              ? `حالة الاتصال: ${data.connected ? "متصل" : "غير متصل"}${data.connected ? "" : " — بانتظار السيرفر"}`
              : `غير متصل بـ ${data.server_url.replace('https://', '').replace('http://', '')}`}
          </div>
        </div>
      </div>

      {/* Website Link */}
      <button onClick={openWebsite} style={{
        width: "100%", padding: "12px", marginBottom: 20,
        background: "white", borderRadius: 12, border: "1px solid #e5e7eb",
        display: "flex", alignItems: "center", justifyContent: "center", gap: 8,
        fontSize: "0.85rem", fontWeight: 600, color: "#2C3E50",
        cursor: "pointer", transition: "all 0.15s",
      }}
      onMouseEnter={(e) => { (e.target as HTMLElement).style.background = "#f9fafb"; }}
      onMouseLeave={(e) => { (e.target as HTMLElement).style.background = "white"; }}
      >
        <i className="fas fa-external-link-alt" style={{ fontSize: "0.8rem" }}></i>
        فتح لوحة التحكم — ordoo.shop
      </button>

      {/* Info Cards */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12, marginBottom: 20 }}>
        <div style={{ background: "white", borderRadius: 12, padding: 16, border: "1px solid #f0f2f5" }}>
          <div style={{ color: "#7a8d9e", fontSize: "0.75rem", marginBottom: 4 }}>المتجر</div>
          <div style={{ fontWeight: 700, color: "#1a2a38" }}>{data.store_name || "—"}</div>
        </div>
        <div style={{ background: "white", borderRadius: 12, padding: 16, border: "1px solid #f0f2f5" }}>
          <div style={{ color: "#7a8d9e", fontSize: "0.75rem", marginBottom: 4 }}>السيرفر</div>
          <div style={{ fontWeight: 700, color: "#1a2a38", fontSize: "0.8rem" }}>{data.server_url.replace('https://', '').replace('http://', '')}</div>
        </div>
        <div style={{ background: "white", borderRadius: 12, padding: 16, border: "1px solid #f0f2f5" }}>
          <div style={{ color: "#7a8d9e", fontSize: "0.75rem", marginBottom: 4 }}>المحطة</div>
          <div style={{ fontWeight: 700, color: "#1a2a38" }}>{data.station_name || "—"}</div>
        </div>
        <div style={{ background: "white", borderRadius: 12, padding: 16, border: "1px solid #f0f2f5" }}>
          <div style={{ color: "#7a8d9e", fontSize: "0.75rem", marginBottom: 4 }}>الإصدار</div>
          <div style={{ fontWeight: 700, color: "#1a2a38" }}>v{data.version}</div>
        </div>
      </div>

      {/* Queue Stats */}
      <div style={{ background: "white", borderRadius: 16, padding: 20, border: "1px solid #f0f2f5", marginBottom: 20 }}>
        <h3 style={{ margin: "0 0 12px", fontSize: "0.9rem", fontWeight: 700 }}>قائمة الطباعة</h3>
        <div style={{ display: "flex", gap: 16 }}>
          <div style={{ textAlign: "center" }}>
            <div style={{ fontSize: "1.5rem", fontWeight: 800, color: "#d97706" }}>{data.queue_stats.pending}</div>
            <div style={{ fontSize: "0.7rem", color: "#7a8d9e" }}>قيد الانتظار</div>
          </div>
          <div style={{ textAlign: "center" }}>
            <div style={{ fontSize: "1.5rem", fontWeight: 800, color: "#2563eb" }}>{data.queue_stats.printing}</div>
            <div style={{ fontSize: "0.7rem", color: "#7a8d9e" }}>جاري الطباعة</div>
          </div>
          <div style={{ textAlign: "center" }}>
            <div style={{ fontSize: "1.5rem", fontWeight: 800, color: "#16a34a" }}>{data.queue_stats.completed}</div>
            <div style={{ fontSize: "0.7rem", color: "#7a8d9e" }}>مكتملة</div>
          </div>
          <div style={{ textAlign: "center" }}>
            <div style={{ fontSize: "1.5rem", fontWeight: 800, color: "#dc2626" }}>{data.queue_stats.failed}</div>
            <div style={{ fontSize: "0.7rem", color: "#7a8d9e" }}>فشلت</div>
          </div>
        </div>
      </div>

      {/* Last Job */}
      {data.last_job && (
        <div style={{ background: "white", borderRadius: 16, padding: 20, border: "1px solid #f0f2f5" }}>
          <h3 style={{ margin: "0 0 8px", fontSize: "0.9rem", fontWeight: 700 }}>آخر وظيفة</h3>
          <div style={{ fontSize: "0.85rem", color: "#1a2a38" }}>
            طلب #{data.last_job.job_id} — <span style={{ color: "#7a8d9e" }}>{data.last_job.status}</span>
          </div>
        </div>
      )}
    </div>
  );
}
