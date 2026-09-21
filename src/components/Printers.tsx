import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface ServerPrinter {
  id: number;
  name: string;
  connection_type: string;
  ip_address: string | null;
  port: number | null;
  section_name: string | null;
}

interface LocalPrinter {
  id: number;
  name: string;
  connection_type: string;
  ip_address: string | null;
  port: number | null;
  paper_width: number | null;
  is_online: boolean;
  last_test_at: string | null;
  last_test_success: boolean | null;
}

export default function Printers() {
  const [serverPrinters, setServerPrinters] = useState<ServerPrinter[]>([]);
  const [localPrinters, setLocalPrinters] = useState<LocalPrinter[]>([]);
  const [testingId, setTestingId] = useState<number | null>(null);
  const [testResults, setTestResults] = useState<{ [key: number]: { success: boolean; message: string } }>({});
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadPrinters();
  }, []);

  async function loadPrinters() {
    setLoading(true);
    try {
      // Fetch printers from server
      const serverData = await invoke<{ success: boolean; printers: ServerPrinter[] }>("get_server_printers");
      if (serverData.success) {
        setServerPrinters(serverData.printers);
      }
    } catch (e) {
      console.error("Failed to load server printers:", e);
    }

    try {
      // Fetch local printers
      const localData = await invoke<LocalPrinter[]>("get_printers");
      setLocalPrinters(localData);
    } catch (e) {
      console.error("Failed to load local printers:", e);
    }
    setLoading(false);
  }

  async function testPrinter(printerId: number) {
    setTestingId(printerId);
    try {
      const result = await invoke<{ success: boolean; connected: boolean; message: string }>(
        "test_server_printer",
        { printerId }
      );
      setTestResults(prev => ({
        ...prev,
        [printerId]: { success: result.connected, message: result.message }
      }));
      // Refresh printers to update status
      await loadPrinters();
    } catch (e: any) {
      setTestResults(prev => ({
        ...prev,
        [printerId]: { success: false, message: e.toString() }
      }));
    } finally {
      setTestingId(null);
    }
  }

  function getConnectionLabel(type: string): string {
    const labels: { [key: string]: string } = {
      usb: "USB", wifi: "واي فاي", bluetooth: "بلوتوث", lan: "شبكة"
    };
    return labels[type] || type;
  }

  if (loading) return <div style={{ textAlign: "center", padding: 40, color: "#7a8d9e" }}>جاري التحميل...</div>;

  return (
    <div style={{ maxWidth: 700, margin: "0 auto" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 20 }}>
        <h1 style={{ fontSize: "1.2rem", fontWeight: 800, color: "#1a2a38", margin: 0 }}>الطابعات</h1>
        <button onClick={loadPrinters} style={{
          padding: "6px 12px", background: "#f3f4f6", color: "#6b7280",
          borderRadius: 8, border: "none", fontSize: "0.8rem", cursor: "pointer",
        }}>
          <i className="fas fa-sync-alt ml-1"></i> تحديث
        </button>
      </div>

      {/* Server Printers */}
      {serverPrinters.length > 0 && (
        <div style={{ marginBottom: 24 }}>
          <h2 style={{ fontSize: "0.85rem", fontWeight: 700, color: "#7a8d9e", marginBottom: 12 }}>
            <i className="fas fa-cloud ml-1"></i> طابعات الموقع
          </h2>
          {serverPrinters.map(p => (
            <div key={p.id} style={{
              background: "white", borderRadius: 12, padding: 16, marginBottom: 8,
              border: "1px solid #f0f2f5", display: "flex", alignItems: "center", justifyContent: "space-between",
            }}>
              <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                <div style={{
                  width: 40, height: 40, borderRadius: 10,
                  background: testResults[p.id]?.success ? "#dcfce7" : (testResults[p.id] ? "#fef2f2" : "#f3f4f6"),
                  display: "flex", alignItems: "center", justifyContent: "center",
                }}>
                  <i className="fas fa-print" style={{
                    color: testResults[p.id]?.success ? "#16a34a" : (testResults[p.id] ? "#dc2626" : "#9ca3af"),
                    fontSize: "0.9rem",
                  }}></i>
                </div>
                <div>
                  <div style={{ fontWeight: 700, fontSize: "0.85rem", color: "#1a2a38" }}>{p.name}</div>
                  <div style={{ fontSize: "0.75rem", color: "#7a8d9e", marginTop: 2 }}>
                    {getConnectionLabel(p.connection_type)}
                    {p.ip_address && ` · ${p.ip_address}:${p.port || 9100}`}
                    {p.connection_type === 'usb' && !p.ip_address && ' · اتصال مباشر'}
                    {p.section_name && <span style={{ color: "#2563eb", marginRight: 8 }}>← {p.section_name}</span>}
                  </div>
                  {testResults[p.id] && (
                    <div style={{
                      fontSize: "0.7rem", marginTop: 4,
                      color: testResults[p.id].success ? "#16a34a" : "#dc2626",
                    }}>
                      {testResults[p.id].message}
                    </div>
                  )}
                </div>
              </div>
              <button
                onClick={() => testPrinter(p.id)}
                disabled={testingId === p.id}
                style={{
                  padding: "6px 14px", borderRadius: 8, border: "none",
                  background: testingId === p.id ? "#e5e7eb" : "#2C3E50",
                  color: testingId === p.id ? "#9ca3af" : "white",
                  fontSize: "0.75rem", fontWeight: 600, cursor: testingId === p.id ? "not-allowed" : "pointer",
                }}
              >
                {testingId === p.id ? (
                  <><i className="fas fa-spinner fa-spin ml-1"></i> جاري...</>
                ) : (
                  <><i className="fas fa-plug ml-1"></i> اختبار</>
                )}
              </button>
            </div>
          ))}
        </div>
      )}

      {/* No printers */}
      {serverPrinters.length === 0 && localPrinters.length === 0 && (
        <div style={{
          background: "white", borderRadius: 16, padding: 40, textAlign: "center",
          border: "1px solid #f0f2f5",
        }}>
          <i className="fas fa-print" style={{ fontSize: "2rem", color: "#d1d5db", marginBottom: 12 }}></i>
          <div style={{ color: "#7a8d9e", fontSize: "0.9rem" }}>لا توجد طابعات</div>
          <div style={{ color: "#9ca3af", fontSize: "0.8rem", marginTop: 4 }}>أضف طابعة من لوحة التحكم على ordoo.shop</div>
        </div>
      )}
    </div>
  );
}
