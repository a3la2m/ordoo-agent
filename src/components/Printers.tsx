import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface SavedPrinter {
  id: number;
  server_printer_id: number | null;
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
  const [printers, setPrinters] = useState<SavedPrinter[]>([]);
  const [showAdd, setShowAdd] = useState(false);
  const [name, setName] = useState("");
  const [connType, setConnType] = useState("network");
  const [ip, setIp] = useState("");
  const [port, setPort] = useState("9100");
  const [paperWidth, setPaperWidth] = useState("80");
  const [testing, setTesting] = useState<number | null>(null);
  const [testResult, setTestResult] = useState<{ id: number; ok: boolean } | null>(null);

  useEffect(() => { loadPrinters(); }, []);

  async function loadPrinters() {
    try {
      const result = await invoke<SavedPrinter[]>("get_printers");
      setPrinters(result);
    } catch (e) { console.error(e); }
  }

  async function handleAdd() {
    if (!name.trim()) return;
    try {
      await invoke("add_printer", {
        name: name.trim(),
        connectionType: connType,
        ipAddress: ip.trim() || null,
        port: parseInt(port) || 9100,
        paperWidth: parseInt(paperWidth) || 80,
      });
      setShowAdd(false);
      setName(""); setIp(""); setPort("9100");
      loadPrinters();
    } catch (e: any) { alert(e.toString()); }
  }

  async function handleRemove(id: number) {
    if (!confirm("Delete this printer?")) return;
    try {
      await invoke("remove_printer", { id });
      loadPrinters();
    } catch (e: any) { alert(e.toString()); }
  }

  async function handleTest(id: number) {
    setTesting(id);
    setTestResult(null);
    try {
      const ok = await invoke<boolean>("test_printer", { id });
      setTestResult({ id, ok });
      loadPrinters();
    } catch (e: any) {
      setTestResult({ id, ok: false });
    } finally {
      setTesting(null);
    }
  }

  const inputStyle: React.CSSProperties = { width: "100%", padding: "10px 14px", border: "1px solid #e5e7eb", borderRadius: 10, fontSize: "0.85rem", outline: "none", boxSizing: "border-box" };

  return (
    <div style={{ maxWidth: 700, margin: "0 auto" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 20 }}>
        <h1 style={{ fontSize: "1.2rem", fontWeight: 800, color: "#1a2a38", margin: 0 }}>الطابعات</h1>
        <button onClick={() => setShowAdd(!showAdd)} style={{ background: "#2C3E50", color: "white", padding: "8px 16px", borderRadius: 10, fontSize: "0.85rem", fontWeight: 700, border: "none", cursor: "pointer" }}>
          {showAdd ? "إلغاء" : "+ إضافة طابعة"}
        </button>
      </div>

      {showAdd && (
        <div style={{ background: "white", borderRadius: 16, padding: 20, border: "1px solid #e5e7eb", marginBottom: 16 }}>
          <h3 style={{ margin: "0 0 16px", fontSize: "0.95rem" }}>إضافة طابعة جديدة</h3>
          <div style={{ marginBottom: 12 }}>
            <label style={{ fontSize: "0.8rem", fontWeight: 600, color: "#4a6070", display: "block", marginBottom: 4 }}>اسم الطابعة</label>
            <input value={name} onChange={e => setName(e.target.value)} placeholder="مثال: طابعة المطبخ" style={inputStyle} />
          </div>
          <div style={{ display: "flex", gap: 12, marginBottom: 12 }}>
            <div style={{ flex: 1 }}>
              <label style={{ fontSize: "0.8rem", fontWeight: 600, color: "#4a6070", display: "block", marginBottom: 4 }}>نوع الاتصال</label>
              <select value={connType} onChange={e => setConnType(e.target.value)} style={inputStyle}>
                <option value="network">شبكة (TCP/IP)</option>
                <option value="usb">USB</option>
                <option value="bluetooth">بلوتوث</option>
              </select>
            </div>
            <div style={{ flex: 1 }}>
              <label style={{ fontSize: "0.8rem", fontWeight: 600, color: "#4a6070", display: "block", marginBottom: 4 }}>الورق</label>
              <select value={paperWidth} onChange={e => setPaperWidth(e.target.value)} style={inputStyle}>
                <option value="80">80mm</option>
                <option value="58">58mm</option>
              </select>
            </div>
          </div>
          {connType === "network" && (
            <div style={{ display: "flex", gap: 12, marginBottom: 16 }}>
              <div style={{ flex: 2 }}>
                <label style={{ fontSize: "0.8rem", fontWeight: 600, color: "#4a6070", display: "block", marginBottom: 4 }}>IP Address</label>
                <input value={ip} onChange={e => setIp(e.target.value)} placeholder="192.168.1.100" style={inputStyle} />
              </div>
              <div style={{ flex: 1 }}>
                <label style={{ fontSize: "0.8rem", fontWeight: 600, color: "#4a6070", display: "block", marginBottom: 4 }}>Port</label>
                <input value={port} onChange={e => setPort(e.target.value)} placeholder="9100" style={inputStyle} />
              </div>
            </div>
          )}
          <button onClick={handleAdd} style={{ width: "100%", background: "#2C3E50", color: "white", padding: "10px", borderRadius: 10, fontSize: "0.85rem", fontWeight: 700, border: "none", cursor: "pointer" }}>
            حفظ
          </button>
        </div>
      )}

      {printers.length === 0 ? (
        <div style={{ background: "white", borderRadius: 16, padding: 32, textAlign: "center", border: "1px solid #f0f2f5" }}>
          <div style={{ fontSize: "2rem", marginBottom: 12 }}>🖨️</div>
          <p style={{ color: "#7a8d9e", margin: 0 }}>لا توجد طابعات مضافة</p>
        </div>
      ) : (
        printers.map(p => (
          <div key={p.id} style={{ background: "white", borderRadius: 16, padding: 16, marginBottom: 8, border: "1px solid #f0f2f5", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <div>
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <span style={{ fontSize: "0.9rem", fontWeight: 700, color: "#1a2a38" }}>{p.name}</span>
                <span style={{ background: p.last_test_success ? "#d1fae5" : "#f5f6f8", color: p.last_test_success ? "#16a34a" : "#7a8d9e", padding: "2px 8px", borderRadius: 6, fontSize: "0.65rem", fontWeight: 600 }}>
                  {p.last_test_success ? "متصل" : "غير مختبر"}
                </span>
              </div>
              <div style={{ fontSize: "0.75rem", color: "#7a8d9e", marginTop: 4 }}>
                {p.connection_type === "network" ? `${p.ip_address}:${p.port}` : p.connection_type} • {p.paper_width || 80}mm
              </div>
            </div>
            <div style={{ display: "flex", gap: 8 }}>
              <button onClick={() => handleTest(p.id)} disabled={testing === p.id} style={{ padding: "6px 12px", borderRadius: 8, border: "1px solid #e5e7eb", background: "white", cursor: "pointer", fontSize: "0.75rem", fontWeight: 600 }}>
                {testing === p.id ? "..." : "اختبار"}
              </button>
              <button onClick={() => handleRemove(p.id)} style={{ padding: "6px 12px", borderRadius: 8, border: "1px solid #fecaca", background: "#fef2f2", color: "#dc2626", cursor: "pointer", fontSize: "0.75rem", fontWeight: 600 }}>
                حذف
              </button>
            </div>
          </div>
        ))
      )}
    </div>
  );
}
