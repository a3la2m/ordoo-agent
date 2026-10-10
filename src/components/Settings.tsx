import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Config {
  paired: boolean;
  store_name: string | null;
  station_name: string | null;
  machine_id: string;
  agent_id: number | null;
  version: string;
  server_url: string;
  store_id: number | null;
  session_user: string | null;
}

export default function Settings({ onChanged }: { onChanged?: () => void }) {
  const [config, setConfig] = useState<Config | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [msgType, setMsgType] = useState<"success" | "error" | "info">("info");
  const [showReconnect, setShowReconnect] = useState(false);
  const [showUnpair, setShowUnpair] = useState(false);
  const [unpairing, setUnpairing] = useState(false);
  const [reconnectToken, setReconnectToken] = useState("");
  const [reconnectLoading, setReconnectLoading] = useState(false);
  const [reconnectError, setReconnectError] = useState<string | null>(null);
  const [checkingConnection, setCheckingConnection] = useState(false);
  const [serverUrl, setServerUrl] = useState("");
  const [savingServer, setSavingServer] = useState(false);

  useEffect(() => { loadConfig(); }, []);

  async function loadConfig() {
    try {
      const cfg = await invoke<Config>("get_config");
      setConfig(cfg);
      setServerUrl(cfg.server_url);
    } catch (e) { console.error(e); }
  }

  // تبديل سيرفر الاختبار (يُسمح فقط عند عدم الارتباط)
  async function handleSaveServer() {
    const url = serverUrl.trim();
    if (!url) { showMsg("أدخل عنوان السيرفر", "error"); return; }
    setSavingServer(true);
    try {
      const res = await invoke<{ server_url: string; ws_url: string }>("set_server_url", { url });
      showMsg(`تم حفظ السيرفر: ${res.server_url}`, "success");
      await loadConfig();
      onChanged?.();
    } catch (e: any) {
      showMsg(String(e), "error");
    } finally {
      setSavingServer(false);
    }
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
        onChanged?.();
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
    // بعد فك الربط سابقًا: لا تُعاد العملية — الزر يتحوّل لصفحة الربط
    if (!config?.paired) { setShowUnpair(false); return; }
    if (unpairing) return;
    setUnpairing(true);
    try {
      const result = await invoke<string>("unpair_with_server");
      showMsg(result, "success");
      setShowUnpair(false);
      await loadConfig();
      onChanged?.(); // تحديث فوري لحالة القائمة الجانبية (إزالة شريحة المستخدم والمتجر)
    } catch (e: any) {
      // Even if server call fails, local unpair
      try {
        await invoke<string>("unpair");
        showMsg("تم فك الربط محلياً", "info");
        setShowUnpair(false);
        await loadConfig();
        onChanged?.();
      } catch (e2: any) {
        showMsg(e2.toString(), "error");
      }
    } finally {
      setUnpairing(false);
    }
  }

  // تسجيل الدخول من الوكيل: نُقلت بطاقته إلى الشاشة الرئيسية (يظهر فقط حين لا جلسة نشطة)

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
          <div><span style={{ color: "#7a8d9e" }}>المحطة:</span> <strong>{config.station_name || "—"}</strong></div>
          <div><span style={{ color: "#7a8d9e" }}>Agent ID:</span> <strong>{config.agent_id || "—"}</strong></div>
          <div><span style={{ color: "#7a8d9e" }}>الإصدار:</span> <strong>v{config.version}</strong></div>
          <div><span style={{ color: "#7a8d9e" }}>الحالة:</span> <strong style={{ color: config.paired ? "#16a34a" : "#dc2626" }}>{config.paired ? "مربوط" : "غير مربوط"}</strong></div>
          <div><span style={{ color: "#7a8d9e" }}>جلسة الدخول:</span> <strong style={{ color: config.session_user ? "#16a34a" : "#7a8d9e" }}>{config.session_user ? `مسجّل باسم ${config.session_user}` : "غير مسجّل"}</strong></div>
        </div>
      </div>

      {/* عنوان السيرفر (تبديل بيئة الاختبار) */}
      <div style={{ background: "white", borderRadius: 16, padding: 20, border: "1px solid #f0f2f5", marginBottom: 16 }}>
        <h3 style={{ margin: "0 0 4px", fontSize: "0.9rem", fontWeight: 700 }}>
          <i className="fas fa-server ml-2" style={{ color: "#2563eb" }}></i>
          عنوان السيرفر
        </h3>
        <p style={{ fontSize: "0.75rem", color: "#7a8d9e", margin: "0 0 10px" }}>
          {config.paired
            ? "البرنامج مرتبط — أزل الربط أولًا لتغيير عنوان السيرفر"
            : "لتبديل بيئة الاختبار: غيّر العنوان ثم اربط بالتوكن من هذه البيئة"}
        </p>

        <div style={{ display: "flex", gap: 8 }}>
          <input
            type="text"
            value={serverUrl}
            onChange={e => setServerUrl(e.target.value)}
            disabled={config.paired || savingServer}
            placeholder="https://ordoo.shop"
            dir="ltr"
            style={{
              flex: 1, padding: "10px 14px", border: "2px solid #e5e7eb",
              borderRadius: 10, fontSize: "0.85rem", fontFamily: "monospace", outline: "none",
              background: config.paired ? "#f8fafc" : "white",
              color: config.paired ? "#94a3b8" : "#1a2a38",
              boxSizing: "border-box",
            }}
          />
          <button
            onClick={handleSaveServer}
            disabled={config.paired || savingServer}
            style={{
              padding: "10px 16px",
              background: config.paired ? "#cbd5e1" : savingServer ? "#a0a8b0" : "#2C3E50",
              color: "white", borderRadius: 10, border: "none", fontWeight: 700,
              cursor: config.paired || savingServer ? "not-allowed" : "pointer",
              whiteSpace: "nowrap",
            }}>
            {savingServer ? "جاري الحفظ..." : "حفظ"}
          </button>
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
        <button
          onClick={() => {
            // فُك الربط سابقًا → لا نافذة فك، بل صفحة ربط المحطة على الويب
            if (!config.paired) {
              const url = `${config.server_url.replace(/\/+$/, "")}/merchant/printing/setup#stations`;
              invoke("open_external", { url }).catch(() => window.open(url, "_blank"));
              return;
            }
            setShowUnpair(true);
          }}
          style={{
            flex: 1, padding: "10px",
            background: config.paired ? "#fef2f2" : "#ecfdf5",
            color: config.paired ? "#dc2626" : "#16a34a",
            borderRadius: 10, border: config.paired ? "1px solid #fecaca" : "1px solid #bbf7d0",
            fontWeight: 700, cursor: "pointer",
          }}>
          {config.paired ? (
            <><i className="fas fa-unlink ml-2"></i> فك الربط</>
          ) : (
            <><i className="fas fa-link ml-2"></i> فتح صفحة ربط المحطة</>
          )}
        </button>
      </div>

      {/* Unpair Warning Modal */}
      {showUnpair && config.paired && (
        <div style={{
          position: "fixed", inset: 0, background: "rgba(0,0,0,0.5)",
          display: "flex", alignItems: "center", justifyContent: "center",
          zIndex: 100, padding: 16,
        }} onClick={() => !unpairing && setShowUnpair(false)}>
          <div style={{
            background: "white", borderRadius: 16, padding: 24,
            width: "100%", maxWidth: 420,
          }} onClick={e => e.stopPropagation()}>
            <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 8 }}>
              <div style={{
                width: 36, height: 36, borderRadius: 10, background: "#fef2f2",
                color: "#dc2626", display: "flex", alignItems: "center", justifyContent: "center",
              }}>
                <i className="fas fa-unlink"></i>
              </div>
              <h3 style={{ margin: 0, fontSize: "1rem", fontWeight: 800 }}>فصل Ordoo Agent عن السيرفر؟</h3>
            </div>
            <ul style={{ fontSize: "0.8rem", color: "#4b5563", margin: "0 0 16px", paddingInlineStart: 18, lineHeight: 2 }}>
              <li>سيُسجَّل خروج المستخدم <b>من هذا البرنامج فقط</b> (جلسة الويب تبقى كما هي)</li>
              <li>ستُحذف هذه المحطة من قائمة المحطات في لوحة التاجر</li>
              <li style={{ color: "#dc2626", fontWeight: 700 }}>
                <i className="fas fa-print" style={{ marginLeft: 4 }}></i>
                لن تتمكن من الطباعة من البرنامج مباشرة — طابعات هذه المحطة (الفاتورة والأقسام) لن تستلم أي وظيفة حتى تُربط من جديد
              </li>
            </ul>
            <div style={{ display: "flex", gap: 8 }}>
              <button
                onClick={() => setShowUnpair(false)}
                disabled={unpairing}
                style={{
                  flex: 1, padding: "10px", background: "#f3f4f6", color: "#374151",
                  borderRadius: 10, border: "none", fontWeight: 700, cursor: "pointer",
                }}>
                إلغاء
              </button>
              <button
                onClick={handleUnpair}
                disabled={unpairing}
                style={{
                  flex: 1, padding: "10px",
                  background: unpairing ? "#fca5a5" : "#dc2626",
                  color: "white", borderRadius: 10, border: "none", fontWeight: 700,
                  cursor: unpairing ? "not-allowed" : "pointer",
                }}>
                {unpairing ? (
                  <><i className="fas fa-spinner fa-spin ml-2"></i> جاري الفصل...</>
                ) : (
                  <><i className="fas fa-unlink ml-2"></i> نعم، فك الربط</>
                )}
              </button>
            </div>
          </div>
        </div>
      )}

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
