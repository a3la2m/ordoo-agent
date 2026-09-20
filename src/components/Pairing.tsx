import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Props {
  onPaired: () => void;
}

export default function Pairing({ onPaired }: Props) {
  const [token, setToken] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  async function handlePair() {
    if (!token.trim()) {
      setError("يرجى إدخال توكن الربط");
      return;
    }
    setLoading(true);
    setError(null);
    setSuccess(null);
    try {
      const response = await invoke<any>("start_pairing", {
        token: token.trim(),
      });
      if (response.success) {
        setSuccess("تم الربط بنجاح!");
        onPaired();
      } else {
        setError(response.message || "فشل الربط");
      }
    } catch (e: any) {
      setError(e.toString());
    } finally {
      setLoading(false);
    }
  }

  return (
    <div style={{ maxWidth: 450, margin: "60px auto", textAlign: "center" }}>
      <div style={{ width: 80, height: 80, borderRadius: "50%", background: "linear-gradient(135deg, #1a2a38, #2C3E50)", display: "flex", alignItems: "center", justifyContent: "center", margin: "0 auto 20px", fontSize: "2rem" }}>
        🔗
      </div>
      <h1 style={{ fontSize: "1.3rem", fontWeight: 800, color: "#1a2a38", margin: "0 0 8px" }}>
        ربط Ordoo Agent
      </h1>
      <p style={{ color: "#7a8d9e", fontSize: "0.85rem", margin: "0 0 24px" }}>
        أدخل التوكن من صفحة إعداد الطباعة في لوحة التاجر
      </p>

      <div style={{ textAlign: "right", marginBottom: 16 }}>
        <label style={{ display: "block", fontSize: "0.8rem", fontWeight: 600, color: "#4a6070", marginBottom: 6 }}>توكن الربط</label>
        <input
          type="text"
          value={token}
          onChange={(e) => setToken(e.target.value.toUpperCase())}
          placeholder="AB12CD34"
          style={{ width: "100%", padding: "12px 16px", border: "2px solid #e5e7eb", borderRadius: 12, fontSize: "1.1rem", fontFamily: "monospace", letterSpacing: 3, textAlign: "center", outline: "none", boxSizing: "border-box" }}
        />
      </div>

      {error && (
        <div style={{ background: "#fef2f2", color: "#dc2626", padding: "10px 16px", borderRadius: 10, fontSize: "0.85rem", marginBottom: 16 }}>
          {error}
        </div>
      )}

      {success && (
        <div style={{ background: "#f0fdf4", color: "#16a34a", padding: "10px 16px", borderRadius: 10, fontSize: "0.85rem", marginBottom: 16 }}>
          {success}
        </div>
      )}

      <button
        onClick={handlePair}
        disabled={loading}
        style={{
          width: "100%", padding: "12px", background: loading ? "#a0a8b0" : "#2C3E50", color: "white",
          borderRadius: 12, fontSize: "0.95rem", fontWeight: 700, border: "none", cursor: loading ? "not-allowed" : "pointer",
        }}
      >
        {loading ? "جاري الربط..." : "ربط"}
      </button>
    </div>
  );
}
