import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Config {
  paired: boolean;
  store_name: string | null;
  machine_id: string;
  agent_id: number | null;
  version: string;
  server_url: string;
  store_id: number | null;
}

export default function Settings() {
  const [config, setConfig] = useState<Config | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [showReconnect, setShowReconnect] = useState(false);
  const [reconnectToken, setReconnectToken] = useState("");
  const [reconnectLoading, setReconnectLoading] = useState(false);
  const [reconnectError, setReconnectError] = useState<string | null>(null);

  useEffect(() => {
    loadConfig();
  }, []);

  async function loadConfig() {
    try {
      const cfg = await invoke<Config>("get_config");
      setConfig(cfg);
    } catch (e) {
      console.error(e);
    }
  }

  async function handleUnpair() {
    if (!confirm("هل أنت متأكد من فك الربط؟ سيتعطل البرنامج عن العمل حتى تربط جهازًا جديدًا.")) return;
    try {
      const result = await invoke<string>("unpair");
      setMsg(result);
      await loadConfig();
    } catch (e: any) {
      setMsg(e.toString());
    }
  }

  async function handleReconnect() {
    if (!reconnectToken.trim()) {
      setReconnectError("يرجى إدخال توكن الربط");
      return;
    }
    setReconnectLoading(true);
    setReconnectError(null);
    try {
      const response = await invoke<any>("start_pairing", {
        token: reconnectToken.trim(),
      });
      if (response.success) {
        setMsg("تم إعادة الربط بنجاح!");
        setShowReconnect(false);
        setReconnectToken("");
        await loadConfig();
      } else {
        setReconnectError(response.message || "فشل الربط");
      }
    } catch (e: any) {
      setReconnectError(e.toString());
    } finally {
      setReconnectLoading(false);
    }
  }

  if (!config) return <div style={{ textAlign: "center", padding: 40, color: "#7a8d9e" }}>جاري التحميل...</div>;

  return (
    <div style={{ maxWidth: 500, margin: "0 auto" }}>
      <h1 style={{ fontSize: "1.2rem", fontWeight: 800, color: "#1a2a38", margin: "0 0 20px" }}>الإعدادات</h1>

      {msg && (
        <div style={{ background: "#f0f9ff", color: "#2563eb", padding: "10px 16px", borderRadius: 10, fontSize: "0.85rem", marginBottom: 16 }}>
          {msg}
        </div>
      )}

      <div style={{ background: "white", borderRadius: 16, padding: 20, border: "1px solid #f0f2f5", marginBottom: 16 }}>
        <h3 style={{ margin: "0 0 12px", fontSize: "0.9rem", fontWeight: 700 }}>معلومات الجهاز</h3>
        <div style={{ display: "grid", gap: 8, fontSize: "0.85rem" }}>
          <div><span style={{ color: "#7a8d9e" }}>السيرفر:</span> <strong>{config.server_url}</strong></div>
          <div><span style={{ color: "#7a8d9e" }}>Machine ID:</span> <strong style={{ fontSize: "0.75rem", fontFamily: "monospace" }}>{config.machine_id}</strong></div>
          <div><span style={{ color: "#7a8d9e" }}>المتجر:</span> <strong>{config.store_name || "—"}</strong></div>
          <div><span style={{ color: "#7a8d9e" }}>Agent ID:</span> <strong>{config.agent_id || "—"}</strong></div>
          <div><span style={{ color: "#7a8d9e" }}>الإصدار:</span> <strong>v{config.version}</strong></div>
          <div><span style={{ color: "#7a8d9e" }}>الحالة:</span> <strong style={{ color: config.paired ? "#16a34a" : "#dc2626" }}>{config.paired ? "مربوط" : "غير مربوط"}</strong></div>
        </div>
      </div>

      <div style={{ display: "flex", gap: 8 }}>
        <button onClick={() => setShowReconnect(true)} style={{
          flex: 1, padding: "10px", background: "#2C3E50", color: "white",
          borderRadius: 10, border: "none", fontWeight: 700, cursor: "pointer",
        }}>
          <i className="fas fa-redo ml-2"></i> إعادة الاتصال
        </button>
        <button onClick={handleUnpair} style={{
          flex: 1, padding: "10px", background: "#fef2f2", color: "#dc2626",
          borderRadius: 10, border: "1px solid #fecaca", fontWeight: 700, cursor: "pointer",
        }}>
          <i className="fas fa-unlink ml-2"></i> فك الربط
        </button>
      </div>

      {/* Reconnect Modal */}
      {showReconnect && (
        <div style={{
          position: "fixed", inset: 0, background: "rgba(0,0,0,0.5)",
          display: "flex", alignItems: "center", justifyContent: "center",
          zIndex: 100, padding: 16,
        }} onClick={() => setShowReconnect(false)}>
          <div style={{
            background: "white", borderRadius: 16, padding: 24,
            width: "100%", maxWidth: 400,
          }} onClick={(e) => e.stopPropagation()}>
            <h3 style={{ margin: "0 0 8px", fontSize: "1rem", fontWeight: 800 }}>إعادة الربط</h3>
            <p style={{ fontSize: "0.8rem", color: "#7a8d9e", margin: "0 0 16px" }}>
              أدخل التوكن الجديد من صفحة إعداد الطباعة في لوحة التاجر
            </p>

            <div style={{ marginBottom: 12 }}>
              <label style={{ display: "block", fontSize: "0.8rem", fontWeight: 600, color: "#4a6070", marginBottom: 4 }}>توكن الربط</label>
              <input
                type="text"
                value={reconnectToken}
                onChange={(e) => setReconnectToken(e.target.value.toUpperCase())}
                placeholder="AB12CD34"
                style={{
                  width: "100%", padding: "10px 14px", border: "2px solid #e5e7eb",
                  borderRadius: 10, fontSize: "1rem", fontFamily: "monospace",
                  letterSpacing: 2, textAlign: "center", outline: "none",
                  boxSizing: "border-box",
                }}
              />
            </div>

            {reconnectError && (
              <div style={{ background: "#fef2f2", color: "#dc2626", padding: "8px 12px", borderRadius: 8, fontSize: "0.8rem", marginBottom: 12 }}>
                {reconnectError}
              </div>
            )}

            <div style={{ display: "flex", gap: 8 }}>
              <button onClick={handleReconnect} disabled={reconnectLoading} style={{
                flex: 1, padding: "10px",
                background: reconnectLoading ? "#a0a8b0" : "#2C3E50",
                color: "white", borderRadius: 10, border: "none", fontWeight: 700,
                cursor: reconnectLoading ? "not-allowed" : "pointer",
              }}>
                {reconnectLoading ? "جاري الربط..." : "ربط"}
              </button>
              <button onClick={() => { setShowReconnect(false); setReconnectToken(""); setReconnectError(null); }} style={{
                padding: "10px 16px", background: "#f3f4f6", color: "#6b7280",
                borderRadius: 10, border: "none", fontWeight: 600, cursor: "pointer",
              }}>
                إلغاء
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
