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
    if (!confirm("هل أنت متأكد من فك الربط؟")) return;
    try {
      const result = await invoke<string>("unpair");
      setMsg(result);
      loadConfig();
    } catch (e: any) {
      setMsg(e.toString());
    }
  }

  async function handleReconnect() {
    try {
      const result = await invoke<string>("reconnect");
      setMsg(result);
    } catch (e: any) {
      setMsg(e.toString());
    }
  }

  if (!config) return <div>جاري التحميل...</div>;

  return (
    <div style={{ maxWidth: 500, margin: "0 auto" }}>
      <h1 style={{ fontSize: "1.2rem", fontWeight: 800, color: "#1a2a38", margin: "0 0 20px" }}>الإعدادات</h1>

      {msg && (
        <div style={{ background: "#f0f9ff", color: "#2563eb", padding: "10px 16px", borderRadius: 10, fontSize: "0.85rem", marginBottom: 16 }}>
          {msg}
        </div>
      )}

      <div style={{ background: "white", borderRadius: 16, padding: 20, border: "1px solid #f0f2f5", marginBottom: 16 }}>
        <h3 style={{ margin: "0 0 12px", fontSize: "0.9rem" }}>معلومات الجهاز</h3>
        <div style={{ display: "grid", gap: 8, fontSize: "0.85rem" }}>
          <div><span style={{ color: "#7a8d9e" }}>Server:</span> <strong>{config.server_url}</strong></div>
          <div><span style={{ color: "#7a8d9e" }}>Machine ID:</span> <strong style={{ fontSize: "0.75rem", fontFamily: "monospace" }}>{config.machine_id}</strong></div>
          <div><span style={{ color: "#7a8d9e" }}>Store:</span> <strong>{config.store_name || "—"}</strong></div>
          <div><span style={{ color: "#7a8d9e" }}>Agent ID:</span> <strong>{config.agent_id || "—"}</strong></div>
          <div><span style={{ color: "#7a8d9e" }}>Version:</span> <strong>{config.version}</strong></div>
          <div><span style={{ color: "#7a8d9e" }}>Status:</span> <strong style={{ color: config.paired ? "#16a34a" : "#dc2626" }}>{config.paired ? "مربوط" : "غير مربوط"}</strong></div>
        </div>
      </div>

      <div style={{ display: "flex", gap: 8 }}>
        <button onClick={handleReconnect} style={{ flex: 1, padding: "10px", background: "#2C3E50", color: "white", borderRadius: 10, border: "none", fontWeight: 700, cursor: "pointer" }}>
          إعادة الاتصال
        </button>
        <button onClick={handleUnpair} style={{ flex: 1, padding: "10px", background: "#fef2f2", color: "#dc2626", borderRadius: 10, border: "1px solid #fecaca", fontWeight: 700, cursor: "pointer" }}>
          فك الربط
        </button>
      </div>
    </div>
  );
}
