import { useState, useEffect, type ReactNode, type CSSProperties } from "react";
import { invoke } from "@tauri-apps/api/core";

/** إعداد طابعة — USB أو شبكة فقط (لا بلوتوث) */
interface PrinterConfig {
  id?: number | null;
  name: string;
  connection_type: string; // "usb" | "network"
  ip_address?: string | null;
  port?: number | null;
  paper_width?: number | null;
  max_attempts?: number | null;
  device_path?: string | null;
}

interface SectionInfo {
  id: number;
  name: string;
  is_active?: boolean | null;
  printer?: PrinterConfig | null;
}

interface PrintSetup {
  success?: boolean;
  receipt_printer?: PrinterConfig | null;
  sections: SectionInfo[];
}

type Editing =
  | { kind: "receipt" }
  | { kind: "section"; sectionId: number; sectionName: string }
  | null;

interface TestOutcome {
  success: boolean;
  message: string;
}

const emptyForm = {
  name: "",
  connection_type: "usb",
  ip_address: "",
  port: 9100,
  paper_width: 80,
  max_attempts: 3,
  device_path: "",
};

export default function Printers() {
  const [setup, setSetup] = useState<PrintSetup | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [usbNodes, setUsbNodes] = useState<string[]>([]);

  const [editing, setEditing] = useState<Editing>(null);
  const [form, setForm] = useState({ ...emptyForm });
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const [testingKey, setTestingKey] = useState<string | null>(null);
  const [testResults, setTestResults] = useState<Record<string, TestOutcome>>({});

  useEffect(() => {
    loadSetup();
  }, []);

  async function loadSetup() {
    setLoading(true);
    setLoadError(null);
    try {
      const data = await invoke<PrintSetup>("get_print_setup");
      setSetup(data);
    } catch (e) {
      setLoadError(String(e));
    }
    try {
      const nodes = await invoke<string[]>("list_usb_nodes");
      setUsbNodes(nodes);
    } catch {
      setUsbNodes([]);
    }
    setLoading(false);
  }

  function openEditor(target: Editing, cfg?: PrinterConfig | null) {
    setSaveError(null);
    setEditing(target);
    if (cfg) {
      setForm({
        name: cfg.name,
        connection_type: cfg.connection_type === "network" ? "network" : "usb",
        ip_address: cfg.ip_address ?? "",
        port: cfg.port ?? 9100,
        paper_width: cfg.paper_width ?? 80,
        max_attempts: cfg.max_attempts ?? 3,
        device_path: cfg.device_path ?? "",
      });
    } else {
      const base =
        target && target.kind === "section"
          ? `طابعة ${target.sectionName}`
          : "طابعة الفاتورة";
      setForm({ ...emptyForm, name: base });
    }
  }

  function buildConfig(): PrinterConfig {
    const isNetwork = form.connection_type === "network";
    return {
      name: form.name.trim(),
      connection_type: isNetwork ? "network" : "usb",
      ip_address: isNetwork ? form.ip_address.trim() : null,
      port: isNetwork ? Number(form.port) || 9100 : null,
      paper_width: Number(form.paper_width) || 80,
      max_attempts: Number(form.max_attempts) || 3,
      device_path: !isNetwork ? form.device_path || null : null,
    };
  }

  async function saveForm() {
    const cfg = buildConfig();
    if (!cfg.name) {
      setSaveError("أدخل اسم الطابعة");
      return;
    }
    if (cfg.connection_type === "network" && !cfg.ip_address) {
      setSaveError("طابعة الشبكة تتطلب عنوان IP");
      return;
    }

    setSaving(true);
    setSaveError(null);
    try {
      if (editing?.kind === "receipt") {
        const next = await invoke<PrintSetup>("save_receipt_printer", { printer: cfg });
        setSetup(next);
      } else if (editing?.kind === "section") {
        const next = await invoke<PrintSetup>("save_section_printer", {
          sectionId: editing.sectionId,
          printer: cfg,
        });
        setSetup(next);
      }
      setEditing(null);
    } catch (e) {
      setSaveError(String(e));
    }
    setSaving(false);
  }

  async function removeReceiptPrinter() {
    if (!confirm("إزالة طابعة الفاتورة من هذه المحطة؟ (ستُطبع الفواتير من المتصفح)")) return;
    setSaving(true);
    setSaveError(null);
    try {
      const next = await invoke<PrintSetup>("save_receipt_printer", { printer: null });
      setSetup(next);
      setEditing(null);
    } catch (e) {
      setSaveError(String(e));
    }
    setSaving(false);
  }

  async function runTest(key: string, cfg: PrinterConfig) {
    setTestingKey(key);
    try {
      const res = await invoke<{ success: boolean; message: string }>("test_printer_config", {
        connectionType: cfg.connection_type,
        ipAddress: cfg.ip_address ?? null,
        port: cfg.port ?? null,
        devicePath: cfg.device_path ?? null,
      });
      setTestResults(prev => ({ ...prev, [key]: { success: res.success, message: res.message } }));
    } catch (e) {
      setTestResults(prev => ({ ...prev, [key]: { success: false, message: String(e) } }));
    }
    setTestingKey(null);
  }

  function connLabel(cfg: PrinterConfig): string {
    if (cfg.connection_type === "network") {
      return `شبكة · ${cfg.ip_address ?? "?"}:${cfg.port ?? 9100}`;
    }
    return `USB${cfg.device_path ? ` · ${cfg.device_path}` : ""}`;
  }

  if (loading) {
    return <div style={{ textAlign: "center", padding: 40, color: "#7a8d9e" }}>جاري التحميل...</div>;
  }

  return (
    <div style={{ maxWidth: 700, margin: "0 auto" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 12 }}>
        <h1 style={{ fontSize: "1.2rem", fontWeight: 800, color: "#1a2a38", margin: 0 }}>
          الطابعات
        </h1>
        <button onClick={loadSetup} style={{
          padding: "6px 12px", background: "#f3f4f6", color: "#6b7280",
          borderRadius: 8, border: "none", fontSize: "0.8rem", cursor: "pointer",
        }}>
          <i className="fas fa-sync-alt ml-1"></i> تحديث
        </button>
      </div>

      {/* شرح المبدأ */}
      <div style={{
        background: "#f8fafc", border: "1px solid #e2e8f0", borderRadius: 12,
        padding: "10px 14px", fontSize: "0.75rem", color: "#475569", lineHeight: 1.7,
        marginBottom: 16,
      }}>
        <i className="fas fa-circle-info ml-1" style={{ color: "#2563eb" }}></i>
        كل الطابعات تُعرَّف هنا على هذا الجهاز: <b>طابعة الفاتورة</b> الخاصة بهذه المحطة،
        و<b>طابعة واحدة لكل قسم</b> (USB أو IP وبورت، حجم الورق، عدد المحاولات، وزر اختبار).
        الأقسام والمحطات نفسها تُعرَّف من الويب.
      </div>

      {loadError && (
        <div style={{
          background: "#fef2f2", border: "1px solid #fecaca", borderRadius: 12,
          padding: 14, fontSize: "0.8rem", color: "#b91c1c", marginBottom: 16,
        }}>
          <i className="fas fa-triangle-exclamation ml-1"></i> {loadError}
        </div>
      )}

      {/* ─── طابعة الفاتورة الرئيسية ─── */}
      <div style={{ marginBottom: 20 }}>
        <h2 style={{ fontSize: "0.85rem", fontWeight: 700, color: "#7a8d9e", marginBottom: 12 }}>
          <i className="fas fa-receipt ml-1"></i> طابعة الفاتورة الرئيسية
          <span style={{
            fontSize: "0.65rem", background: "#fef3c7", color: "#92400e",
            padding: "1px 8px", borderRadius: 999, marginRight: 8, fontWeight: 700,
          }}>
            تابعة لهذه المحطة وحدها
          </span>
        </h2>

        {editing?.kind === "receipt" ? (
          <EditorCard
            form={form} setForm={setForm} usbNodes={usbNodes}
            saving={saving} saveError={saveError}
            testResults={testResults} testingKey={testingKey}
            testKey="form-receipt"
            onTest={() => runTest("form-receipt", buildConfig())}
            onSave={saveForm}
            onCancel={() => setEditing(null)}
            onRemove={setup?.receipt_printer ? removeReceiptPrinter : undefined}
          />
        ) : setup?.receipt_printer ? (
          <SummaryCard
            title={setup.receipt_printer.name}
            badge={<span style={{
              background: "#fef3c7", color: "#92400e", fontWeight: 700,
              fontSize: "0.65rem", padding: "1px 8px", borderRadius: 999, marginRight: 8,
            }}>فاتورة هذه المحطة</span>}
            details={[
              connLabel(setup.receipt_printer),
              `ورق ${setup.receipt_printer.paper_width ?? 80}mm`,
              `${setup.receipt_printer.max_attempts ?? 3} محاولات`,
            ]}
            test={testResults["receipt"]}
            testing={testingKey === "receipt"}
            onTest={() => runTest("receipt", setup.receipt_printer!)}
            onEdit={() => openEditor({ kind: "receipt" }, setup.receipt_printer)}
          />
        ) : (
          <div style={{
            background: "white", borderRadius: 12, padding: 20, marginBottom: 8,
            border: "1px dashed #f59e0b", display: "flex", justifyContent: "space-between",
            alignItems: "center", gap: 12, flexWrap: "wrap",
          }}>
            <div style={{ fontSize: "0.8rem", color: "#92400e" }}>
              <i className="fas fa-circle-exclamation ml-1"></i>
              لم تُعرَّف بعد — بدونها تُطبع الفواتير من المتصفح فقط
            </div>
            <button
              onClick={() => openEditor({ kind: "receipt" })}
              style={{
                padding: "8px 16px", borderRadius: 8, border: "none",
                background: "#2C3E50", color: "white", fontSize: "0.78rem",
                fontWeight: 700, cursor: "pointer",
              }}
            >
              <i className="fas fa-plus ml-1"></i> تعريف الطابعة
            </button>
          </div>
        )}
      </div>

      {/* ─── طابعات الأقسام ─── */}
      <div style={{ marginBottom: 24 }}>
        <h2 style={{ fontSize: "0.85rem", fontWeight: 700, color: "#7a8d9e", marginBottom: 12 }}>
          <i className="fas fa-layer-group ml-1"></i> طابعات الأقسام
          <span style={{ fontSize: "0.68rem", fontWeight: 500, marginRight: 8 }}>
            — كل قسم له طابعته، والأقسام مشتركة بين المحطات
          </span>
        </h2>

        {!setup || setup.sections.length === 0 ? (
          <div style={{
            background: "white", borderRadius: 16, padding: 30, textAlign: "center",
            border: "1px solid #f0f2f5",
          }}>
            <i className="fas fa-layer-group" style={{ fontSize: "1.6rem", color: "#d1d5db", marginBottom: 10 }}></i>
            <div style={{ color: "#7a8d9e", fontSize: "0.85rem" }}>لا توجد أقسام طباعة</div>
            <div style={{ color: "#9ca3af", fontSize: "0.75rem", marginTop: 4 }}>
              أنشئ الأقسام من الويب: إعداد الطباعة ← أقسام الطباعة
            </div>
          </div>
        ) : (
          setup.sections.map(sec => (
            <div key={sec.id} style={{ marginBottom: 8 }}>
              {editing?.kind === "section" && editing.sectionId === sec.id ? (
                <>
                  <div style={{
                    fontSize: "0.75rem", fontWeight: 700, color: "#2563eb", marginBottom: 6,
                  }}>
                    <i className="fas fa-pen ml-1"></i> طابعة قسم «{sec.name}»
                  </div>
                  <EditorCard
                    form={form} setForm={setForm} usbNodes={usbNodes}
                    saving={saving} saveError={saveError}
                    testResults={testResults} testingKey={testingKey}
                    testKey={`form-section-${sec.id}`}
                    onTest={() => runTest(`form-section-${sec.id}`, buildConfig())}
                    onSave={saveForm}
                    onCancel={() => setEditing(null)}
                  />
                </>
              ) : (
                <SummaryCard
                  title={sec.name}
                  badge={sec.is_active === false ? (
                    <span style={{
                      fontSize: "0.65rem", background: "#f3f4f6", color: "#6b7280",
                      padding: "1px 8px", borderRadius: 999, marginRight: 8, fontWeight: 700,
                    }}>معطّل</span>
                  ) : undefined}
                  details={
                    sec.printer
                      ? [connLabel(sec.printer), `ورق ${sec.printer.paper_width ?? 80}mm`, `${sec.printer.max_attempts ?? 3} محاولات`]
                      : ["لم تُعرَّف طابعة لهذا القسم بعد"]
                  }
                  missing={!sec.printer}
                  test={testResults[`section-${sec.id}`]}
                  testing={testingKey === `section-${sec.id}`}
                  onTest={sec.printer ? () => runTest(`section-${sec.id}`, sec.printer!) : undefined}
                  onEdit={() => openEditor({ kind: "section", sectionId: sec.id, sectionName: sec.name }, sec.printer)}
                  editLabel={sec.printer ? "تعديل" : "تعريف"}
                />
              )}
            </div>
          ))
        )}
      </div>
    </div>
  );
}

/** بطاقة ملخّص طابعة محفوظة */
function SummaryCard(props: {
  title: string;
  badge?: ReactNode;
  details: string[];
  missing?: boolean;
  test?: TestOutcome;
  testing: boolean;
  onTest?: () => void;
  onEdit: () => void;
  editLabel?: string;
}) {
  return (
    <div style={{
      background: "white", borderRadius: 12, padding: 16,
      border: props.missing ? "1px dashed #e5e7eb" : "1px solid #f0f2f5",
      display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12,
      flexWrap: "wrap",
    }}>
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <div style={{
          width: 40, height: 40, borderRadius: 10,
          background: props.test?.success ? "#dcfce7" : (props.test ? "#fef2f2" : "#f3f4f6"),
          display: "flex", alignItems: "center", justifyContent: "center",
        }}>
          <i className={props.missing ? "fas fa-print" : "fas fa-print"} style={{
            color: props.test?.success ? "#16a34a" : (props.test ? "#dc2626" : (props.missing ? "#d1d5db" : "#9ca3af")),
            fontSize: "0.9rem",
          }}></i>
        </div>
        <div>
          <div style={{ fontWeight: 700, fontSize: "0.85rem", color: props.missing ? "#9ca3af" : "#1a2a38" }}>
            {props.title}
            {props.badge}
          </div>
          <div style={{ fontSize: "0.75rem", color: "#7a8d9e", marginTop: 2 }}>
            {props.details.join(" · ")}
          </div>
          {props.test && (
            <div style={{
              fontSize: "0.7rem", marginTop: 4, whiteSpace: "pre-line",
              color: props.test.success ? "#16a34a" : "#dc2626",
            }}>
              {props.test.message}
            </div>
          )}
        </div>
      </div>
      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
        {props.onTest && (
          <button
            onClick={props.onTest}
            disabled={props.testing}
            style={{
              padding: "6px 14px", borderRadius: 8, border: "1px solid #e5e7eb",
              background: props.testing ? "#e5e7eb" : "white",
              color: props.testing ? "#9ca3af" : "#2563eb",
              fontSize: "0.75rem", fontWeight: 600,
              cursor: props.testing ? "not-allowed" : "pointer",
            }}
          >
            {props.testing ? <><i className="fas fa-spinner fa-spin ml-1"></i> جاري...</> : <><i className="fas fa-plug ml-1"></i> اختبار</>}
          </button>
        )}
        <button
          onClick={props.onEdit}
          style={{
            padding: "6px 14px", borderRadius: 8, border: "none",
            background: "#2C3E50", color: "white",
            fontSize: "0.75rem", fontWeight: 600, cursor: "pointer",
          }}
        >
          <i className="fas fa-pen ml-1"></i> {props.editLabel ?? "تعديل"}
        </button>
      </div>
    </div>
  );
}

/** نموذج تعريف/تعديل طابعة */
function EditorCard(props: {
  form: typeof emptyForm;
  setForm: (f: typeof emptyForm) => void;
  usbNodes: string[];
  saving: boolean;
  saveError: string | null;
  testResults: Record<string, TestOutcome>;
  testingKey: string | null;
  testKey: string;
  onTest: () => void;
  onSave: () => void;
  onCancel: () => void;
  onRemove?: () => void;
}) {
  const { form, setForm, usbNodes } = props;
  const isNetwork = form.connection_type === "network";
  const test = props.testResults[props.testKey];

  const inputStyle: CSSProperties = {
    width: "100%", padding: "8px 10px", borderRadius: 8,
    border: "1px solid #e5e7eb", fontSize: "0.8rem", color: "#1a2a38",
    background: "white", boxSizing: "border-box",
  };

  return (
    <div style={{
      background: "white", borderRadius: 12, padding: 18,
      border: "1px solid #dbeafe",
    }}>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12, marginBottom: 12 }}>
        <label style={{ gridColumn: "1 / -1", fontSize: "0.72rem", fontWeight: 700, color: "#475569" }}>
          اسم الطابعة
          <input
            type="text" value={form.name}
            onChange={e => setForm({ ...form, name: e.target.value })}
            placeholder="مثال: طابعة المشروبات"
            style={{ ...inputStyle, marginTop: 4, fontWeight: 500 }}
          />
        </label>

        <label style={{ fontSize: "0.72rem", fontWeight: 700, color: "#475569" }}>
          نوع الاتصال
          <select
            value={form.connection_type}
            onChange={e => setForm({ ...form, connection_type: e.target.value })}
            style={{ ...inputStyle, marginTop: 4 }}
          >
            <option value="usb">USB (مباشر)</option>
            <option value="network">شبكة (IP وبورت)</option>
          </select>
        </label>

        <label style={{ fontSize: "0.72rem", fontWeight: 700, color: "#475569" }}>
          عرض الورق
          <select
            value={form.paper_width}
            onChange={e => setForm({ ...form, paper_width: Number(e.target.value) })}
            style={{ ...inputStyle, marginTop: 4 }}
          >
            <option value={58}>58mm</option>
            <option value={80}>80mm</option>
          </select>
        </label>

        {isNetwork ? (
          <>
            <label style={{ fontSize: "0.72rem", fontWeight: 700, color: "#475569" }}>
              عنوان IP
              <input
                type="text" value={form.ip_address}
                onChange={e => setForm({ ...form, ip_address: e.target.value })}
                placeholder="192.168.1.100"
                style={{ ...inputStyle, marginTop: 4 }}
              />
            </label>
            <label style={{ fontSize: "0.72rem", fontWeight: 700, color: "#475569" }}>
              البورت
              <input
                type="number" value={form.port}
                onChange={e => setForm({ ...form, port: Number(e.target.value) })}
                style={{ ...inputStyle, marginTop: 4 }}
              />
            </label>
          </>
        ) : (
          <label style={{ gridColumn: "1 / -1", fontSize: "0.72rem", fontWeight: 700, color: "#475569" }}>
            جهاز USB
            <select
              value={form.device_path}
              onChange={e => setForm({ ...form, device_path: e.target.value })}
              style={{ ...inputStyle, marginTop: 4 }}
            >
              <option value="">تلقائي (أول جهاز متاح)</option>
              {usbNodes.map(n => <option key={n} value={n}>{n}</option>)}
              {form.device_path && !usbNodes.includes(form.device_path) && (
                <option value={form.device_path}>{form.device_path}</option>
              )}
            </select>
          </label>
        )}

        <label style={{ gridColumn: "1 / -1", fontSize: "0.72rem", fontWeight: 700, color: "#475569" }}>
          عدد محاولات الطباعة
          <input
            type="number" min={1} max={10} value={form.max_attempts}
            onChange={e => setForm({ ...form, max_attempts: Number(e.target.value) })}
            style={{ ...inputStyle, marginTop: 4 }}
          />
        </label>
      </div>

      {test && (
        <div style={{
          fontSize: "0.75rem", padding: "8px 12px", borderRadius: 8, marginBottom: 12,
          background: test.success ? "#dcfce7" : "#fef2f2",
          color: test.success ? "#166534" : "#b91c1c", fontWeight: 600,
          whiteSpace: "pre-line", wordBreak: "break-word",
        }}>
          <i className={`fas ${test.success ? "fa-check-circle" : "fa-times-circle"} ml-1`}></i>
          {test.message}
        </div>
      )}

      <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
        <button
          onClick={props.onTest}
          disabled={props.testingKey === props.testKey}
          style={{
            padding: "8px 16px", borderRadius: 8, border: "1px solid #bfdbfe",
            background: props.testingKey === props.testKey ? "#e5e7eb" : "#eff6ff",
            color: props.testingKey === props.testKey ? "#9ca3af" : "#2563eb",
            fontSize: "0.78rem", fontWeight: 700,
            cursor: props.testingKey === props.testKey ? "not-allowed" : "pointer",
          }}
        >
          {props.testingKey === props.testKey
            ? <><i className="fas fa-spinner fa-spin ml-1"></i> جاري الاختبار...</>
            : <><i className="fas fa-plug ml-1"></i> اختبار</>}
        </button>
        <button
          onClick={props.onSave}
          disabled={props.saving}
          style={{
            padding: "8px 18px", borderRadius: 8, border: "none",
            background: props.saving ? "#9ca3af" : "#2C3E50", color: "white",
            fontSize: "0.78rem", fontWeight: 700,
            cursor: props.saving ? "not-allowed" : "pointer",
          }}
        >
          {props.saving ? <><i className="fas fa-spinner fa-spin ml-1"></i> جاري الحفظ...</> : <><i className="fas fa-save ml-1"></i> حفظ</>}
        </button>
        <button
          onClick={props.onCancel}
          disabled={props.saving}
          style={{
            padding: "8px 16px", borderRadius: 8, border: "1px solid #e5e7eb",
            background: "white", color: "#6b7280",
            fontSize: "0.78rem", fontWeight: 700, cursor: "pointer",
          }}
        >
          إلغاء
        </button>
        {props.onRemove && (
          <button
            onClick={props.onRemove}
            disabled={props.saving}
            style={{
              padding: "8px 16px", borderRadius: 8, border: "1px solid #fecaca",
              background: "#fef2f2", color: "#dc2626",
              fontSize: "0.78rem", fontWeight: 700, cursor: "pointer",
              marginRight: "auto",
            }}
          >
            <i className="fas fa-trash-alt ml-1"></i> إزالة
          </button>
        )}
      </div>

      {props.saveError && (
        <div style={{
          marginTop: 12, fontSize: "0.75rem", color: "#b91c1c",
          background: "#fef2f2", padding: "8px 12px", borderRadius: 8,
        }}>
          <i className="fas fa-triangle-exclamation ml-1"></i> {props.saveError}
        </div>
      )}
    </div>
  );
}
