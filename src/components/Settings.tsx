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
  const [msgType, setMsgType] = useState<"success" | "error" | "info">("info");
  const [showReconnect, setShowReconnect] = useState(false);
  const [reconnectToken, setReconnectToken] = useState("");
  const [reconnectLoading, setReconnectLoading] = useState(false);
  const [reconnectError, setReconnectError] = useState<string | null>(null);
  const [checkingConnection, setCheckingConnection] = useState(false);

  useEffect(() => { loadConfig(); }, []);

  async function loadConfig() {
    try {
      const cfg = await invoke<Config>("get_config");
      setConfig(cfg);
    } catch (e) { console.error(e); }
  }

  function showMsg(text: string, type: "success" | "error" | "info" = "info") {
    setMsg(text);
    setMsgType(type);
    setTimeout(() => setMsg(null), 5000);
  }

  // If paired → just check connection
  async function handleReconnect() {
    if (!config?.paired) {
      // Not paired → show token modal
      setShowReconnect(true);
      return;
    }

    // Paired → check connection
    setCheckingConnection(true);
    try {
      const result = await invoke<{ success: boolean; message: string }>("check_connection");
      showMsg(result.message, result.success ? "success" : "error");
    } catch (e: any) {
      showMsg("فشل الاتصال بالسيرفر: " + e.toString(), "error");
    } finally {
      setCheckingConnection(false);
    }
  }

  // Pair with new token
  async function handlePairWithToken() {
    if (!reconnectToken.trim()) {
      setReconnectError("يرجى إدخال توكن الربط");
      return;
    }
    setReconnectLoading(true);
    setReconnectError(null);
    try {
      const response = await invoke<any>("start_pairing", { token: reconnectToken.trim() });
      if (response.success) {
        setShowReconnect(false);
        setReconnectToken("");
        showMsg("تم الربط بنجاح!", "success");
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

  async function handleUnpair() {
    if (!confirm("هل أنت متأكد من فك الربط؟\nسيتم حذف هذا الجهاز من الأجهزة المربوطة.")) return;
    try {
      const result = await invoke<string>("unpair_with_server");
      showMsg(result, "success");
      await loadConfig();
    } catch (e: any) {
      // Even if server call fails, local unpair
      try {
        await invoke<string>("unpair");
        showMsg("تم فك الربط محلياً", "info");
        await loadConfig();
      } catch (e2: any) {
        showMsg(e2.toString(), "error");
      }
    }
  }

  if (!config) return <div style={{ textAlign: "center", padding: 40, color: "#7a8d9e" }}>جاري التحميل...</div>;

  const msgColors = { success: "#dcfce7", error: "#fef2f2", info: "#f0f9ff" };
  const msgTextColors = { success: "#16a34a", error: "#dc2626", info: "#2563eb" };

  return (
    <div style={{ maxWidth: 500, margin: "0 auto" }}>
      <h1 style={{ fontSize: "1.2rem", fontWeight: 800, color: "#1a2a38", margin: "0 0 20px" }}>الإعدادات</h1>

      {msg && (
        <div style={{ background: msgColors[msgType], color: msgTextColors[msgType], padding: "10px 16px", borderRadius: 10, fontSize: "0.85rem", marginBottom: 16 }}>
          <i className={`fas ${msgType === "success" ? "fa-check-circle" : msgType === "error" ? "fa-exclamation-circle" : "fa-info-circle"} ml-2`}></i>
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
        <button onClick={handleReconnect} disabled={checkingConnection} style={{
          flex: 1, padding: "10px",
          background: checkingConnection ? "#a0a8b0" : "#2C3E50",
          color: "white", borderRadius: 10, border: "none", fontWeight: 700,
          cursor: checkingConnection ? "not-allowed" : "pointer",
        }}>
          {checkingConnection ? (
            <><i className="fas fa-spinner fa-spin ml-2"></i> جاري الفحص...</>
          ) : config.paired ? (
            <><i className="fas fa-plug ml-2"></i> فحص الاتصال</>
          ) : (
            <><i className="fas fa-redo ml-2"></i> إعادة الاتصال</>
          )}
        </button>
        <button onClick={handleUnpair} style={{
          flex: 1, padding: "10px", background: "#fef2f2", color: "#dc2626",
          borderRadius: 10, border: "1px solid #fecaca", fontWeight: 700, cursor: "pointer",
        }}>
          <i className="fas fa-unlink ml-2"></i> فك الربط
        </button>
      </div>

      {/* Reconnect/Pair Modal */}
      {showReconnect && (
        <div style={{
          position: "fixed", inset: 0, background: "rgba(0,0,0,0.5)",
          display: "flex", alignItems: "center", justifyContent: "center",
          zIndex: 100, padding: 16,
        }} onClick={() => setShowReconnect(false)}>
          <div style={{
            background: "white", borderRadius: 16, padding: 24,
            width: "100%", maxWidth: 400,
          }} onClick={e => e.stopPropagation()}>
            <h3 style={{ margin: "0 0 8px", fontSize: "1rem", fontWeight: 800 }}>ربط الجهاز</h3>
            <p style={{ fontSize: "0.8rem", color: "#7a8d9e", margin: "0 0 16px" }}>
              أدخل التوكن من صفحة إعداد الطباعة في لوحة التاجر
            </p>

            <div style={{ marginBottom: 12 }}>
              <input
                type="text" value={reconnectToken}
                onChange={e => setReconnectToken(e.target.value.toUpperCase())}
                placeholder="AB12CD34"
                style={{
                  width: "100%", padding: "10px 14px", border: "2px solid #e5e7eb",
                  borderRadius: 10, fontSize: "1rem", fontFamily: "monospace",
                  letterSpacing: 2, textAlign: "center", outline: "none", boxSizing: "border-box",
                }}
              />
            </div>

            {reconnectError && (
              <div style={{ background: "#fef2f2", color: "#dc2626", padding: "8px 12px", borderRadius: 8, fontSize: "0.8rem", marginBottom: 12 }}>
                {reconnectError}
              </div>
            )}

            <div style={{ display: "flex", gap: 8 }}>
              <button onClick={handlePairWithToken} disabled={reconnectLoading} style={{
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
              }}>إلغاء</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
