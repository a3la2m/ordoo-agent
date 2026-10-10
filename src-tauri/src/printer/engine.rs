use serde::{Deserialize, Serialize};

/// Payload from Laravel (snapshot)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintPayload {
    pub order_number: String,
    pub store_name: String,
    pub section_name: String,
    pub created_at: String,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
    pub notes: Option<String>,
    pub channel: Option<String>,
    pub items: Vec<PrintItem>,

    // إجماليات الفاتورة — توجد في وظيفة الفاتورة فقط (اختيارية في وظائف الأقسام)
    #[serde(default)]
    pub subtotal: Option<f64>,
    #[serde(default)]
    pub discount_total: Option<f64>,
    #[serde(default)]
    pub delivery_fee: Option<f64>,
    #[serde(default)]
    pub total: Option<f64>,
    #[serde(default)]
    pub paid_cash: Option<f64>,
    #[serde(default)]
    pub paid_card: Option<f64>,
    #[serde(default)]
    pub change: Option<f64>,
    #[serde(default)]
    pub payment_method: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintItem {
    pub product_name: String,
    pub quantity: f64,
    pub price: f64,
    pub unit: Option<String>,
    pub notes: Option<String>,
    pub addons: Vec<serde_json::Value>,
}

pub struct PrintEngine;

impl PrintEngine {
    /// Convert payload JSON string to ESC/POS bytes
    pub fn render_escpos(payload_json: &str, paper_width: u32) -> Result<Vec<u8>, String> {
        let payload: PrintPayload = serde_json::from_str(payload_json)
            .map_err(|e| format!("Invalid payload JSON: {}", e))?;

        let mut buf = Vec::new();

        // Initialize printer
        buf.extend_from_slice(b"\x1B\x40");

        // Set code page to Arabic
        buf.extend_from_slice(b"\x1B\x74\x17");

        // Center alignment
        buf.extend_from_slice(b"\x1B\x61\x01");

        // Store name (bold, large)
        buf.extend_from_slice(b"\x1B\x45\x01"); // Bold ON
        Self::push_str(&mut buf, &format!("{}\n", payload.store_name));
        buf.extend_from_slice(b"\x1B\x45\x00"); // Bold OFF

        // Section name
        Self::push_str(&mut buf, &format!("{}\n", payload.section_name));

        // Divider
        Self::push_str(&mut buf, &Self::divider(paper_width));

        // Left alignment
        buf.extend_from_slice(b"\x1B\x61\x00");

        // Order number + date
        Self::push_str(&mut buf, &format!("Order: {}  {}\n", payload.order_number, payload.created_at));

        if let Some(ref name) = payload.customer_name {
            Self::push_str(&mut buf, &format!("Customer: {}\n", name));
        }

        // Divider
        Self::push_str(&mut buf, &Self::divider(paper_width));

        // Items
        for item in &payload.items {
            // Product name + qty
            let line = format!("{} x{} {}\n",
                item.product_name,
                item.quantity,
                item.unit.as_deref().unwrap_or(""),
            );
            Self::push_str(&mut buf, &line);

            // Price
            Self::push_str(&mut buf, &format!("  {:.2}\n", item.price));

            // Addons
            for addon in &item.addons {
                if let Some(name) = addon.get("name").and_then(|v| v.as_str()) {
                    Self::push_str(&mut buf, &format!("  + {}\n", name));
                }
            }

            // Item notes
            if let Some(ref notes) = item.notes {
                Self::push_str(&mut buf, &format!("  Note: {}\n", notes));
            }
        }

        // Totals block (يُطبع فقط عندما توجد الإجماليات — وظيفة الفاتورة)
        if payload.subtotal.is_some()
            || payload.total.is_some()
            || payload.paid_cash.is_some()
            || payload.paid_card.is_some()
        {
            buf.extend_from_slice(b"\x1B\x61\x00"); // Left align
            if let Some(v) = payload.subtotal {
                Self::push_str(&mut buf, &format!("Subtotal: {:.2}\n", v));
            }
            if let Some(v) = payload.discount_total {
                if v != 0.0 {
                    Self::push_str(&mut buf, &format!("Discount: -{:.2}\n", v));
                }
            }
            if let Some(v) = payload.delivery_fee {
                if v != 0.0 {
                    Self::push_str(&mut buf, &format!("Delivery: {:.2}\n", v));
                }
            }
            if let Some(t) = payload.total {
                Self::push_str(&mut buf, &format!("TOTAL: {:.2}\n", t));
            }
            if let Some(v) = payload.paid_cash {
                if v > 0.0 {
                    Self::push_str(&mut buf, &format!("Cash: {:.2}\n", v));
                }
            }
            if let Some(v) = payload.paid_card {
                if v > 0.0 {
                    Self::push_str(&mut buf, &format!("Card: {:.2}\n", v));
                }
            }
            if let Some(v) = payload.change {
                if v > 0.0 {
                    Self::push_str(&mut buf, &format!("Change: {:.2}\n", v));
                }
            }
            if let Some(ref m) = payload.payment_method {
                Self::push_str(&mut buf, &format!("Method: {}\n", m));
            }
        }

        // Divider
        Self::push_str(&mut buf, &Self::divider(paper_width));

        // Notes
        if let Some(ref notes) = payload.notes {
            Self::push_str(&mut buf, &format!("Notes: {}\n", notes));
        }

        // Footer
        buf.extend_from_slice(b"\x1B\x61\x01"); // Center
        Self::push_str(&mut buf, "Thank you\n");
        Self::push_str(&mut buf, &Self::divider(paper_width));

        // Feed + cut
        buf.extend_from_slice(b"\n\n\n");
        buf.extend_from_slice(b"\x1D\x56\x00"); // Full cut

        Ok(buf)
    }

    fn push_str(buf: &mut Vec<u8>, s: &str) {
        // Attempt UTF-8 to Windows-1256 for Arabic on ESC/POS
        // Fallback to lossy ASCII
        for c in s.chars() {
            if c.is_ascii() {
                buf.push(c as u8);
            } else if is_arabic(c) {
                // Simple Arabic character mapping for ESC/POS
                // In production, use proper Arabic shaping library
                buf.push(b'?');
            } else {
                buf.push(b'?');
            }
        }
    }

    fn divider(paper_width: u32) -> String {
        let len = if paper_width >= 80 { 48 } else { 32 };
        "-".repeat(len)
    }
}

fn is_arabic(c: char) -> bool {
    let cp = c as u32;
    (0x0600..=0x06FF).contains(&cp) || (0x0750..=0x077F).contains(&cp)
}
