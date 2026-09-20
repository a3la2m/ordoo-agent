import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface LogEntry {
  id: number;
  level: string;
  category: string;
  message: string;
  details: string | null;
  created_at: string;
}

export default function Logs() {
  const [logs, setLogs] = useState<LogEntry[]>([]);

  useEffect(() => {
    loadLogs();
  }, []);

  async function loadLogs() {
    try {
      const result = await invoke<LogEntry[]>("get_logs", { limit: 100 });
      setLogs(result);
    } catch (e) {
      console.error(e);
    }
  }

  const levelColors: Record<string, string> = {
    info: "#2563eb",
    warn: "#d97706",
    error: "#dc2626",
  };

  return (
    <div style={{ maxWidth: 700, margin: "0 auto" }}>
      <h1 style={{ fontSize: "1.2rem", fontWeight: 800, color: "#1a2a38", margin: "0 0 20px" }}>السجل</h1>

      {logs.length === 0 ? (
        <div style={{ background: "white", borderRadius: 16, padding: 32, textAlign: "center", border: "1px solid #f0f2f5" }}>
          <p style={{ color: "#7a8d9e", margin: 0 }}>لا توجد سجلات</p>
        </div>
      ) : (
        <div style={{ background: "white", borderRadius: 16, border: "1px solid #f0f2f5", overflow: "hidden" }}>
          {logs.map((log) => (
            <div key={log.id} style={{ padding: "10px 16px", borderBottom: "1px solid #f5f6f8" }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <span style={{ width: 8, height: 8, borderRadius: "50%", background: levelColors[log.level] || "#7a8d9e", flexShrink: 0 }}></span>
                <span style={{ fontWeight: 600, color: "#1a2a38", fontSize: "0.85rem" }}>{log.message}</span>
                <span style={{ fontSize: "0.7rem", color: "#a0a8b0", marginRight: "auto" }}>{log.created_at}</span>
              </div>
              <div style={{ fontSize: "0.7rem", color: "#7a8d9e", marginTop: 2, marginRight: 16 }}>{log.category}</div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
