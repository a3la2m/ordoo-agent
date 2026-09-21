import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
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
  machine_id: string;
  agent_id: number | null;
  version: string;
  server_url: string;
}

function App() {
  const [page, setPage] = useState<Page>("dashboard");
  const [config, setConfig] = useState<AgentConfig | null>(null);

  useEffect(() => {
    loadConfig();
  }, []);

  async function loadConfig() {
    try {
      const cfg = await invoke<AgentConfig>("get_config");
      setConfig(cfg);
      if (!cfg.paired) {
        setPage("pairing");
      } else {
        setPage("dashboard");
      }
    } catch (e) {
      console.error("Failed to load config:", e);
    }
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
            style={{ width: 36, height: 36, borderRadius: 8 }}
            onError={(e) => {
              // Fallback if logo not found
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
          {config?.paired ? (
            <div>
              <div style={{ color: "#7A9170", fontWeight: 600 }}>{config.store_name || "متصل"}</div>
              <div style={{ color: "#666", marginTop: 2 }}>ID: {config.agent_id}</div>
            </div>
          ) : (
            <div style={{ color: "#d97706", fontWeight: 600 }}>غير مربوط</div>
          )}
        </div>
      </aside>

      {/* Main Content */}
      <main style={{ flex: 1, overflow: "auto", padding: 24 }}>
        {page === "dashboard" && <Dashboard />}
        {page === "pairing" && <Pairing onPaired={handlePaired} />}
        {page === "printers" && <Printers />}
        {page === "queue" && <Queue />}
        {page === "logs" && <Logs />}
        {page === "settings" && <Settings />}
      </main>
    </div>
  );
}

export default App;
