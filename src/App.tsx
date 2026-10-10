import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Dashboard from "./components/Dashboard";
import Pairing from "./components/Pairing";
import Printers from "./components/Printers";
import Queue from "./components/Queue";
import Logs from "./components/Logs";
import Settings from "./components/Settings";

type Page = "dashboard" | "pairing" | "printers" | "queue" | "logs" | "settings";

interface AgentConfig {
  paired: boolean;
  store_name: string | null;
  station_name: string | null;
  machine_id: string;
  agent_id: number | null;
  version: string;
  server_url: string;
  session_user: string | null;
}

function App() {
  const [page, setPage] = useState<Page>("dashboard");
  const [config, setConfig] = useState<AgentConfig | null>(null);
  const [toastMsg, setToastMsg] = useState<string | null>(null);

  useEffect(() => {
    loadConfig();

    // نتيجة الرابط العميق من الويب: ordoo-agent://pair?... أو ordoo-agent://login?...
    const unLink = listen<{ success: boolean; message: string }>("agent://deep-link-result", (event) => {
      showToast(event.payload.message);
      loadConfig(); // ينقل تلقائيًا: مربوط → الرئيسية، غير مربوط → الربط
    });

    // تغيّر جلسة المستخدم (دخول من الويب/البطاقة، أو خروج من الويب) — بدون نقل الصفحة
    const unSession = listen<{ message: string; user: string | null }>("agent://session-changed", (event) => {
      showToast(event.payload.message);
      loadConfig(false);
    });

    // حُذفت المحطة من السيرفر (توكن مرفوض 401) — فك ارتباط تلقائي ونقل لصفحة الربط
    const unUnpair = listen<{ message: string }>("agent://unpaired", (event) => {
      showToast(event.payload.message);
      loadConfig(); // غير مربوط → صفحة الربط
    });

    // تحديث اسم المحطة/المتجر من النبض — يظهر فورًا في الشريط الجانبي
    const unCfg = listen("agent://config-changed", () => loadConfig(false));

    return () => {
      unLink.then((f) => f());
      unSession.then((f) => f());
      unUnpair.then((f) => f());
      unCfg.then((f) => f());
    };
  }, []);

  function showToast(message: string) {
    setToastMsg(message);
    setTimeout(() => setToastMsg(null), 8000);
  }

  async function loadConfig(navigate = true) {
    try {
      const cfg = await invoke<AgentConfig>("get_config");
      setConfig(cfg);
      if (navigate) {
        if (!cfg.paired) {
          setPage("pairing");
        } else {
          setPage("dashboard");
        }
      }
    } catch (e) {
      console.error("Failed to load config:", e);
    }
  }

  // خروج من البرنامج → يُلغي جلسة الويب أيضًا ( عبر السيرفر)
  async function handleSessionLogout() {
    try {
      const msg = await invoke<string>("agent_session_logout");
      showToast(msg);
    } catch (e) {
      showToast(String(e));
    }
    loadConfig(false);
  }

  function handlePaired() {
    loadConfig(); // Reload config and navigate to dashboard
  }

  const navItems: { id: Page; label: string; icon: string; requiresPairing?: boolean }[] = [
    { id: "dashboard", label: "الرئيسية", icon: "fas fa-home" },
    { id: "printers", label: "الطابعات", icon: "fas fa-print" },
    { id: "queue", label: "القائمة", icon: "fas fa-list" },
    { id: "logs", label: "السجل", icon: "fas fa-history" },
    { id: "settings", label: "الإعدادات", icon: "fas fa-cog" },
  ];

  return (
    <div style={{ display: "flex", height: "100vh", direction: "rtl", fontFamily: "'Segoe UI', Tahoma, sans-serif", background: "#f0f2f5" }}>
      {/* إشعار نتيجة الرابط العميق / تغيّر الجلسة */}
      {toastMsg && (
        <div style={{
          position: "fixed", top: 16, left: "50%", transform: "translateX(-50%)",
          background: "#1a2a38", color: "white", padding: "10px 20px",
          borderRadius: 12, fontSize: "0.85rem", fontWeight: 700, zIndex: 9999,
          boxShadow: "0 6px 24px rgba(0,0,0,0.25)", maxWidth: "90%",
        }}>
          <i className="fas fa-link ml-2" style={{ color: "#7A9170" }}></i>
          {toastMsg}
        </div>
      )}
      {/* Sidebar */}
      <aside style={{
        width: 220,
        background: "#1a2a38",
        color: "white",
        display: "flex",
        flexDirection: "column",
        flexShrink: 0,
      }}>
        {/* Logo */}
        <div style={{
          padding: "20px 16px",
          borderBottom: "1px solid rgba(255,255,255,0.1)",
          display: "flex",
          alignItems: "center",
          gap: 12,
        }}>
          <img
            src="/ordoo-logo.png"
            alt="Ordoo"
            style={{ width: 44, height: 44, borderRadius: 12 }}
            onError={(e) => {
              (e.target as HTMLImageElement).style.display = "none";
            }}
          />
          <div>
            <h1 style={{ fontSize: "1rem", fontWeight: 800, margin: 0 }}>Ordoo Agent</h1>
            <p style={{ fontSize: "0.65rem", color: "#7a9170", margin: "2px 0 0" }}>v{config?.version || "0.1.0"}</p>
          </div>
        </div>

        <nav style={{ flex: 1, padding: "8px 0" }}>
          {navItems.map((item) => {
            if (item.requiresPairing && !config?.paired) return null;
            return (
              <button
                key={item.id}
                onClick={() => setPage(item.id)}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 10,
                  width: "100%",
                  padding: "10px 16px",
                  background: page === item.id ? "rgba(122,145,112,0.2)" : "transparent",
                  color: page === item.id ? "#7A9170" : "#a0a8b0",
                  border: "none",
                  cursor: "pointer",
                  fontSize: "0.85rem",
                  fontWeight: page === item.id ? 700 : 500,
                  transition: "all 0.15s",
                  textAlign: "right",
                }}
              >
                <i className={item.icon} style={{ width: 20, textAlign: "center" }}></i>
                {item.label}
              </button>
            );
          })}
        </nav>

        {/* Status Footer */}
        <div style={{ padding: "12px 16px", borderTop: "1px solid rgba(255,255,255,0.1)", fontSize: "0.7rem", color: "#555" }}>
          {config?.session_user && (
            <div style={{
              display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8,
              padding: "8px 10px", marginBottom: 8, borderRadius: 10,
              background: "rgba(122,145,112,0.12)", border: "1px solid rgba(122,145,112,0.25)",
            }}>
              <span style={{ color: "#7A9170", fontWeight: 700, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                <i className="fas fa-user-check" style={{ marginLeft: 6 }}></i>
                {config.session_user}
              </span>
              <button
                onClick={handleSessionLogout}
                title="تسجيل الخروج من البرنامج (يُخرجك من الويب أيضًا)"
                style={{
                  background: "rgba(217,119,6,0.15)", border: "none", color: "#d97706",
                  borderRadius: 8, padding: "4px 9px", fontSize: "0.65rem", fontWeight: 700,
                  cursor: "pointer", flexShrink: 0,
                }}
              >
                خروج
              </button>
            </div>
          )}
          {config?.paired ? (
            <div>
              <div style={{ color: "#7A9170", fontWeight: 600 }}>{config.station_name || config.store_name || "متصل"}</div>
              <div style={{ color: "#666", marginTop: 2 }}>ID: {config.agent_id}</div>
            </div>
          ) : (
            <div style={{ color: "#d97706", fontWeight: 600 }}>غير مربوط</div>
          )}
        </div>
      </aside>

      {/* Main Content */}
      <main style={{ flex: 1, overflow: "auto", padding: 24 }}>
        {page === "dashboard" && <Dashboard config={config} />}
        {page === "pairing" && <Pairing onPaired={handlePaired} />}
        {page === "printers" && <Printers />}
        {page === "queue" && <Queue />}
        {page === "logs" && <Logs />}
        {page === "settings" && <Settings onChanged={() => loadConfig(false)} />}
      </main>
    </div>
  );
}

export default App;
