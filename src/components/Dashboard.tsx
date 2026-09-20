import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface DashboardData {
  paired: boolean;
  store_name: string | null;
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

  if (!data) return <div style={{ textAlign: "center", padding: 40, color: "#7a8d9e" }}>جاري التحميل...</div>;

  return (
    <div style={{ maxWidth: 700, margin: "0 auto" }}>
      {/* Connection Status */}
      <div style={{
        background: data.paired ? "linear-gradient(135deg, #16a34a, #22c55e)" : "#fef3c7",
        borderRadius: 16, padding: "20px 24px", marginBottom: 20,
        display: "flex", alignItems: "center", gap: 16,
      }}>
        <div style={{
          width: 48, height: 48, borderRadius: "50%",
          background: data.paired ? "rgba(255,255,255,0.2)" : "#fbbf24",
          display: "flex", alignItems: "center", justifyContent: "center",
          fontSize: "1.5rem",
        }}>
          {data.paired ? "🟢" : "🟡"}
        </div>
        <div>
          <div style={{ color: data.paired ? "white" : "#92400e", fontSize: "1.1rem", fontWeight: 800 }}>
            {data.paired ? "متصل" : "غير مربوط"}
          </div>
          <div style={{ color: data.paired ? "rgba(255,255,255,0.8)" : "#b45309", fontSize: "0.8rem" }}>
            WebSocket: {data.ws_status}
          </div>
        </div>
      </div>

      {/* Info Cards */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12, marginBottom: 20 }}>
        <div style={{ background: "white", borderRadius: 12, padding: 16, border: "1px solid #f0f2f5" }}>
          <div style={{ color: "#7a8d9e", fontSize: "0.75rem", marginBottom: 4 }}>المتجر</div>
          <div style={{ fontWeight: 700, color: "#1a2a38" }}>{data.store_name || "—"}</div>
        </div>
        <div style={{ background: "white", borderRadius: 12, padding: 16, border: "1px solid #f0f2f5" }}>
          <div style={{ color: "#7a8d9e", fontSize: "0.75rem", marginBottom: 4 }}>الجهاز</div>
          <div style={{ fontWeight: 700, color: "#1a2a38" }}>{data.machine_name || "—"}</div>
        </div>
        <div style={{ background: "white", borderRadius: 12, padding: 16, border: "1px solid #f0f2f5" }}>
          <div style={{ color: "#7a8d9e", fontSize: "0.75rem", marginBottom: 4 }}>Agent ID</div>
          <div style={{ fontWeight: 700, color: "#1a2a38" }}>{data.agent_id || "—"}</div>
        </div>
        <div style={{ background: "white", borderRadius: 12, padding: 16, border: "1px solid #f0f2f5" }}>
          <div style={{ color: "#7a8d9e", fontSize: "0.75rem", marginBottom: 4 }}>Version</div>
          <div style={{ fontWeight: 700, color: "#1a2a38" }}>{data.version}</div>
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
