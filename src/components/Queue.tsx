import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Job {
  id: number;
  job_id: number;
  store_id: number;
  status: string;
  payload: string;
  attempts: number;
  last_error: string | null;
  created_at: string;
}

const STATUS_COLORS: Record<string, [string, string]> = {
  received: ["#fef3c7", "#d97706"],
  queued: ["#fef3c7", "#d97706"],
  acknowledged: ["#dbeafe", "#2563eb"],
  printing: ["#e0e7ff", "#4f46e5"],
  printed: ["#d1fae5", "#16a34a"],
  failed: ["#fef2f2", "#dc2626"],
};

const STATUS_LABELS: Record<string, string> = {
  received: "مستلم",
  queued: "في القائمة",
  acknowledged: "مؤكد",
  printing: "جاري الطباعة",
  printed: "تمت الطباعة",
  failed: "فشل",
};

export default function Queue() {
  const [jobs, setJobs] = useState<Job[]>([]);

  useEffect(() => {
    loadJobs();
    const interval = setInterval(loadJobs, 3000);
    return () => clearInterval(interval);
  }, []);

  async function loadJobs() {
    try {
      const result = await invoke<Job[]>("get_queue_jobs");
      setJobs(result);
    } catch (e) {
      console.error(e);
    }
  }

  return (
    <div style={{ maxWidth: 700, margin: "0 auto" }}>
      <h1 style={{ fontSize: "1.2rem", fontWeight: 800, color: "#1a2a38", margin: "0 0 20px" }}>قائمة الطباعة</h1>

      {jobs.length === 0 ? (
        <div style={{ background: "white", borderRadius: 16, padding: 32, textAlign: "center", border: "1px solid #f0f2f5" }}>
          <div style={{ fontSize: "2rem", marginBottom: 12 }}>📋</div>
          <p style={{ color: "#7a8d9e", margin: 0 }}>القائمة فارغة</p>
        </div>
      ) : (
        jobs.map((job) => {
          const colors = STATUS_COLORS[job.status] || ["#f5f6f8", "#7a8d9e"];
          return (
            <div key={job.id} style={{ background: "white", borderRadius: 12, padding: "12px 16px", marginBottom: 8, border: "1px solid #f0f2f5", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div>
                <span style={{ fontWeight: 700, color: "#1a2a38" }}>طلب #{job.job_id}</span>
                <span style={{ background: colors[0], color: colors[1], padding: "2px 8px", borderRadius: 6, fontSize: "0.7rem", fontWeight: 600, marginRight: 8 }}>
                  {STATUS_LABELS[job.status] || job.status}
                </span>
              </div>
              <div style={{ fontSize: "0.75rem", color: "#7a8d9e" }}>
                محاولة {job.attempts} • {job.created_at}
              </div>
            </div>
          );
        })
      )}
    </div>
  );
}
