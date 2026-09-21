import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface DashboardData {
  paired: boolean;
  store_name: string | null;
  server_url: string;
  machine_name: string;
  agent_id: number | null;
  version: string;
  ws_status: string;
  printers: any[];
  queue_stats: { pending: number; printing: number; completed: number; failed: number };
  last_job: any | null;
}

export default function Dashboard() {
  const [data, setData] = useState<DashboardData | null>(null);

  useEffect(() => {
    loadData();
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

  function openWebsite() {
    window.open("https://ordoo.shop", "_blank");
  }

  if (!data) return <div style={{ textAlign: "center", padding: 40, color: "#7a8d9e" }}>جاري التحميل...</div>;

  return (
    <div style={{ maxWidth: 700, margin: "0 auto" }}>
      {/* Connection Status */}
      <div style={{
        background: data.paired ? "linear-gradient(135deg, #16a34a, #22c55e)" : "linear-gradient(135deg, #d97706, #f59e0b)",
        borderRadius: 16, padding: "20px 24px", marginBottom: 20,
        display: "flex", alignItems: "center", gap: 16,
      }}>
        <div style={{
          width: 48, height: 48, borderRadius: "50%",
          background: "rgba(255,255,255,0.2)",
          display: "flex", alignItems: "center", justifyContent: "center",
          fontSize: "1.5rem",
        }}>
          {data.paired ? "🟢" : "🟡"}
        </div>
        <div>
          <div style={{ color: "white", fontSize: "1.1rem", fontWeight: 800 }}>
            {data.paired ? "متصل" : "غير مربوط"}
          </div>
          <div style={{ color: "rgba(255,255,255,0.85)", fontSize: "0.8rem" }}>
            {data.paired ? `WebSocket: ${data.ws_status}` : `غير متصل بـ ${data.server_url.replace('https://', '').replace('http://', '')}`}
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
          <div style={{ color: "#7a8d9e", fontSize: "0.75rem", marginBottom: 4 }}>Agent ID</div>
          <div style={{ fontWeight: 700, color: "#1a2a38" }}>{data.agent_id || "—"}</div>
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
