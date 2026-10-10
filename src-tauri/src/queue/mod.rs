use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobStatus {
    Received,
    Queued,
    Acknowledged,
    Printing,
    Printed,
    Failed,
}

impl JobStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Received => "received",
            Self::Queued => "queued",
            Self::Acknowledged => "acknowledged",
            Self::Printing => "printing",
            Self::Printed => "printed",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "received" => Self::Received,
            "queued" => Self::Queued,
            "acknowledged" => Self::Acknowledged,
            "printing" => Self::Printing,
            "printed" => Self::Printed,
            "failed" => Self::Failed,
            _ => Self::Queued,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintJob {
    pub id: i64,
    pub job_id: i64,
    pub store_id: i64,
    pub printer_id: Option<i64>,
    pub section_id: Option<i64>,
    pub status: JobStatus,
    pub payload: String,
    pub attempts: u32,
    pub max_attempts: u32,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateJob {
    pub job_id: i64,
    pub store_id: i64,
    pub printer_id: Option<i64>,
    pub section_id: Option<i64>,
    pub payload: String,
    pub max_attempts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStats {
    pub pending: u32,
    pub printing: u32,
    pub completed: u32,
    pub failed: u32,
}

pub fn init_queue_db(db: &Connection) -> SqlResult<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS print_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            job_id INTEGER NOT NULL UNIQUE,
            store_id INTEGER NOT NULL,
            printer_id INTEGER,
            section_id INTEGER,
            status TEXT NOT NULL DEFAULT 'received',
            payload TEXT NOT NULL,
            attempts INTEGER NOT NULL DEFAULT 0,
            max_attempts INTEGER NOT NULL DEFAULT 3,
            last_error TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_pq_status ON print_queue(status);
        CREATE INDEX IF NOT EXISTS idx_pq_job_id ON print_queue(job_id);
        CREATE INDEX IF NOT EXISTS idx_pq_store_id ON print_queue(store_id);

        CREATE TABLE IF NOT EXISTS printers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            server_printer_id INTEGER,
            name TEXT NOT NULL,
            connection_type TEXT NOT NULL DEFAULT 'network',
            ip_address TEXT,
            port INTEGER DEFAULT 9100,
            paper_width INTEGER DEFAULT 80,
            hosted INTEGER,
            device_path TEXT,
            is_online INTEGER NOT NULL DEFAULT 0,
            last_test_at TEXT,
            last_test_success INTEGER,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_printers_connection ON printers(connection_type);"
    )?;

    // هجرة القواعد القديمة (م3): أعمدة الاستضافة ومسار جهاز USB — الخطأ متجاهل إن كانت موجودة
    let _ = db.execute("ALTER TABLE printers ADD COLUMN hosted INTEGER", []);
    let _ = db.execute("ALTER TABLE printers ADD COLUMN device_path TEXT", []);

    Ok(())
}

/// حارس الاستضافة (م3): يحق لهذا الجهاز طباعة طابعة سيرفرية فقط إن كان هو المضيف لها.
/// - `Some(false)` = السيرفر صرّح بأنها ليست من هذا الجهاز ← يُمنع الطباعة.
/// - `None` = سيرفر لا يرسل هذه الحالة (نسخة أقدم) ← لا منع (توافق خلفي).
/// - طابعات محلية بلا `server_printer_id` ← خارج نطاق الحارس.
pub fn printer_is_hosted_here(p: &SavedPrinter) -> bool {
    !(p.server_printer_id.is_some() && p.hosted == Some(false))
}

pub struct PrintQueue;

impl PrintQueue {
    pub fn enqueue(db: &Connection, job: &CreateJob) -> SqlResult<PrintJob> {
        let existing: SqlResult<i64> = db.query_row(
            "SELECT id FROM print_queue WHERE job_id = ?1",
            params![job.job_id],
            |row| row.get(0),
        );
        if existing.is_ok() {
            return Self::get_by_job_id(db, job.job_id);
        }

        db.execute(
            "INSERT INTO print_queue (job_id, store_id, printer_id, section_id, status, payload, max_attempts)
             VALUES (?1, ?2, ?3, ?4, 'received', ?5, ?6)",
            params![job.job_id, job.store_id, job.printer_id, job.section_id, job.payload, job.max_attempts],
        )?;
        Self::get_by_job_id(db, job.job_id)
    }

    pub fn get_by_job_id(db: &Connection, job_id: i64) -> SqlResult<PrintJob> {
        db.query_row(
            "SELECT id, job_id, store_id, printer_id, section_id, status, payload, attempts, max_attempts, last_error, created_at, updated_at
             FROM print_queue WHERE job_id = ?1",
            params![job_id],
            |row| Ok(PrintJob {
                id: row.get(0)?,
                job_id: row.get(1)?,
                store_id: row.get(2)?,
                printer_id: row.get(3)?,
                section_id: row.get(4)?,
                status: JobStatus::from_str(&row.get::<_, String>(5)?),
                payload: row.get(6)?,
                attempts: row.get::<_, u32>(7)?,
                max_attempts: row.get::<_, u32>(8)?,
                last_error: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            }),
        )
    }

    pub fn get_pending(db: &Connection) -> SqlResult<Vec<PrintJob>> {
        let mut stmt = db.prepare(
            "SELECT id, job_id, store_id, printer_id, section_id, status, payload, attempts, max_attempts, last_error, created_at, updated_at
             FROM print_queue WHERE status IN ('received', 'queued') ORDER BY created_at ASC"
        )?;
        let rows = stmt.query_map([], |row| Ok(PrintJob {
            id: row.get(0)?,
            job_id: row.get(1)?,
            store_id: row.get(2)?,
            printer_id: row.get(3)?,
            section_id: row.get(4)?,
            status: JobStatus::from_str(&row.get::<_, String>(5)?),
            payload: row.get(6)?,
            attempts: row.get::<_, u32>(7)?,
            max_attempts: row.get::<_, u32>(8)?,
            last_error: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        }))?;
        rows.collect()
    }

    pub fn update_status(db: &Connection, job_id: i64, status: JobStatus, error: Option<&str>) -> SqlResult<()> {
        db.execute(
            "UPDATE print_queue SET status = ?1, last_error = ?2, updated_at = datetime('now') WHERE job_id = ?3",
            params![status.label(), error, job_id],
        )?;
        Ok(())
    }

    pub fn increment_attempts(db: &Connection, job_id: i64) -> SqlResult<()> {
        db.execute(
            "UPDATE print_queue SET attempts = attempts + 1, updated_at = datetime('now') WHERE job_id = ?1",
            params![job_id],
        )?;
        Ok(())
    }

    pub fn count_pending(db: &Connection) -> SqlResult<u32> {
        db.query_row(
            "SELECT COUNT(*) FROM print_queue WHERE status IN ('received', 'queued')",
            [], |row| row.get(0),
        )
    }

    pub fn stats(db: &Connection) -> SqlResult<QueueStats> {
        let pending = db.query_row("SELECT COUNT(*) FROM print_queue WHERE status IN ('received', 'queued')", [], |r| r.get::<_, u32>(0))?;
        let printing = db.query_row("SELECT COUNT(*) FROM print_queue WHERE status = 'printing'", [], |r| r.get::<_, u32>(0))?;
        let completed = db.query_row("SELECT COUNT(*) FROM print_queue WHERE status = 'printed'", [], |r| r.get::<_, u32>(0))?;
        let failed = db.query_row("SELECT COUNT(*) FROM print_queue WHERE status = 'failed'", [], |r| r.get::<_, u32>(0))?;
        Ok(QueueStats { pending, printing, completed, failed })
    }

    pub fn get_all(db: &Connection) -> SqlResult<Vec<PrintJob>> {
        let mut stmt = db.prepare(
            "SELECT id, job_id, store_id, printer_id, section_id, status, payload, attempts, max_attempts, last_error, created_at, updated_at
             FROM print_queue ORDER BY created_at DESC LIMIT 100"
        )?;
        let rows = stmt.query_map([], |row| Ok(PrintJob {
            id: row.get(0)?,
            job_id: row.get(1)?,
            store_id: row.get(2)?,
            printer_id: row.get(3)?,
            section_id: row.get(4)?,
            status: JobStatus::from_str(&row.get::<_, String>(5)?),
            payload: row.get(6)?,
            attempts: row.get::<_, u32>(7)?,
            max_attempts: row.get::<_, u32>(8)?,
            last_error: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        }))?;
        rows.collect()
    }

    // --- Printer CRUD ---

    pub fn save_printer(db: &Connection, p: &SavedPrinter) -> SqlResult<SavedPrinter> {
        if p.id == 0 {
            db.execute(
                "INSERT INTO printers (server_printer_id, name, connection_type, ip_address, port, paper_width, hosted, device_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![p.server_printer_id, p.name, p.connection_type, p.ip_address, p.port, p.paper_width, p.hosted, p.device_path],
            )?;
            let id = db.last_insert_rowid();
            Ok(SavedPrinter { id, ..p.clone() })
        } else {
            db.execute(
                "UPDATE printers SET name=?1, connection_type=?2, ip_address=?3, port=?4, paper_width=?5, hosted=?6, device_path=?7 WHERE id=?8",
                params![p.name, p.connection_type, p.ip_address, p.port, p.paper_width, p.hosted, p.device_path, p.id],
            )?;
            Ok(p.clone())
        }
    }

    pub fn get_printers(db: &Connection) -> SqlResult<Vec<SavedPrinter>> {
        let mut stmt = db.prepare(
            "SELECT id, server_printer_id, name, connection_type, ip_address, port, paper_width, hosted, device_path, is_online, last_test_at, last_test_success
             FROM printers ORDER BY name"
        )?;
        let rows = stmt.query_map([], |row| Ok(SavedPrinter {
            id: row.get(0)?,
            server_printer_id: row.get(1)?,
            name: row.get(2)?,
            connection_type: row.get(3)?,
            ip_address: row.get(4)?,
            port: row.get(5)?,
            paper_width: row.get(6)?,
            hosted: row.get::<_, Option<i32>>(7)?.map(|v| v != 0),
            device_path: row.get(8)?,
            is_online: row.get::<_, i32>(9)? != 0,
            last_test_at: row.get(10)?,
            last_test_success: row.get::<_, Option<i32>>(11)?.map(|v| v != 0),
        }))?;
        rows.collect()
    }

    pub fn delete_printer(db: &Connection, id: i64) -> SqlResult<()> {
        db.execute("DELETE FROM printers WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn get_printer_by_id(db: &Connection, id: i64) -> SqlResult<SavedPrinter> {
        db.query_row(
            "SELECT id, server_printer_id, name, connection_type, ip_address, port, paper_width, hosted, device_path, is_online, last_test_at, last_test_success
             FROM printers WHERE id = ?1",
            params![id],
            |row| Ok(SavedPrinter {
                id: row.get(0)?,
                server_printer_id: row.get(1)?,
                name: row.get(2)?,
                connection_type: row.get(3)?,
                ip_address: row.get(4)?,
                port: row.get(5)?,
                paper_width: row.get(6)?,
                hosted: row.get::<_, Option<i32>>(7)?.map(|v| v != 0),
                device_path: row.get(8)?,
                is_online: row.get::<_, i32>(9)? != 0,
                last_test_at: row.get(10)?,
                last_test_success: row.get::<_, Option<i32>>(11)?.map(|v| v != 0),
            }),
        )
    }

    /// البحث بمعرّف الطابعة في السيرفر (الوظيفة تحمل printer_id من السيرفر دائمًا)
    pub fn get_printer_by_server_id(db: &Connection, server_id: i64) -> SqlResult<SavedPrinter> {
        db.query_row(
            "SELECT id, server_printer_id, name, connection_type, ip_address, port, paper_width, hosted, device_path, is_online, last_test_at, last_test_success
             FROM printers WHERE server_printer_id = ?1",
            params![server_id],
            |row| Ok(SavedPrinter {
                id: row.get(0)?,
                server_printer_id: row.get(1)?,
                name: row.get(2)?,
                connection_type: row.get(3)?,
                ip_address: row.get(4)?,
                port: row.get(5)?,
                paper_width: row.get(6)?,
                hosted: row.get::<_, Option<i32>>(7)?.map(|v| v != 0),
                device_path: row.get(8)?,
                is_online: row.get::<_, i32>(9)? != 0,
                last_test_at: row.get(10)?,
                last_test_success: row.get::<_, Option<i32>>(11)?.map(|v| v != 0),
            }),
        )
    }

    /// تعيين صريح لجهاز USB (يُفضَّل على البحث التلقائي «أول جهاز متاح»)
    pub fn set_printer_device_path(db: &Connection, id: i64, device_path: Option<&str>) -> SqlResult<()> {
        db.execute(
            "UPDATE printers SET device_path = ?1 WHERE id = ?2",
            params![device_path, id],
        )?;
        Ok(())
    }

    /// عرض عرض الورق لكل طابعة (58 أو 80)
    pub fn set_printer_paper_width(db: &Connection, id: i64, paper_width: u32) -> SqlResult<()> {
        db.execute(
            "UPDATE printers SET paper_width = ?1 WHERE id = ?2",
            params![paper_width, id],
        )?;
        Ok(())
    }

    pub fn update_printer_status(db: &Connection, id: i64, online: bool, test_ok: Option<bool>) -> SqlResult<()> {
        db.execute(
            "UPDATE printers SET is_online = ?1, last_test_at = datetime('now'), last_test_success = ?2 WHERE id = ?3",
            params![online as i32, test_ok.map(|v| v as i32), id],
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedPrinter {
    pub id: i64,
    pub server_printer_id: Option<i64>,
    pub name: String,
    pub connection_type: String,
    pub ip_address: Option<String>,
    pub port: Option<u16>,
    pub paper_width: Option<u32>,
    /// هل السيرفر صرّح بأن هذه الطابعة مستضافة على هذا الجهاز؟
    /// `None` = سيرفر لا يرسل الحالة (نسخة أقدم) — الحارس لا يمنع عندها.
    pub hosted: Option<bool>,
    /// تعيين صريح لجهاز USB (مثل /dev/usb/lp1) بدل «أول جهاز متاح»
    pub device_path: Option<String>,
    pub is_online: bool,
    pub last_test_at: Option<String>,
    pub last_test_success: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn printer(hosted: Option<bool>, server_printer_id: Option<i64>) -> SavedPrinter {
        SavedPrinter {
            id: 0, // 0 = صف جديد (مسار INSERT في save_printer)
            server_printer_id,
            name: "طابعة الاختبار".to_string(),
            connection_type: "usb".to_string(),
            ip_address: None,
            port: None,
            paper_width: Some(80),
            hosted,
            device_path: None,
            is_online: true,
            last_test_at: None,
            last_test_success: None,
        }
    }

    #[test]
    fn guard_allows_printer_hosted_on_this_station() {
        assert!(printer_is_hosted_here(&printer(Some(true), Some(7))));
    }

    #[test]
    fn guard_blocks_printer_hosted_on_another_station() {
        // م3: طابعة مستضافة على جهاز آخر لا تُطبع هنا مطلقًا
        assert!(!printer_is_hosted_here(&printer(Some(false), Some(7))));
    }

    #[test]
    fn guard_allows_when_server_does_not_send_hosted_state() {
        // توافق خلفي: سيرفر أقدم بلا حقل hosted لا يمنع الطباعة
        assert!(printer_is_hosted_here(&printer(None, Some(7))));
    }

    #[test]
    fn guard_ignores_local_only_printers() {
        // طابعة محلية يدوية (بلا ربط بالسيرفر) خارج نطاق الحارس
        assert!(printer_is_hosted_here(&printer(Some(false), None)));
    }

    fn sample() -> SavedPrinter {
        printer(Some(true), Some(7))
    }

    #[test]
    fn printer_columns_roundtrip() {
        let db = Connection::open_in_memory().unwrap();
        init_queue_db(&db).unwrap();

        let saved = save_printer_with(&db, SavedPrinter {
            hosted: Some(false),
            device_path: Some("/dev/usb/lp1".to_string()),
            paper_width: Some(58),
            ..sample()
        });

        let by_server = PrintQueue::get_printer_by_server_id(&db, 7).unwrap();
        assert_eq!(by_server.hosted, Some(false));
        assert_eq!(by_server.device_path.as_deref(), Some("/dev/usb/lp1"));
        assert_eq!(by_server.paper_width, Some(58));

        let by_id = PrintQueue::get_printer_by_id(&db, saved.id).unwrap();
        assert_eq!(by_id.hosted, Some(false));

        // تعديل المسار يدويًا
        PrintQueue::set_printer_device_path(&db, saved.id, Some("/dev/usb/lp0")).unwrap();
        PrintQueue::set_printer_paper_width(&db, saved.id, 80).unwrap();
        let updated = PrintQueue::get_printers(&db).unwrap().remove(0);
        assert_eq!(updated.device_path.as_deref(), Some("/dev/usb/lp0"));
        assert_eq!(updated.paper_width, Some(80));
    }

    #[test]
    fn legacy_database_gains_new_columns() {
        let db = Connection::open_in_memory().unwrap();
        // بناء القاعدة بالهيكل القديم (بلا hosted/device_path)
        db.execute_batch(
            "CREATE TABLE printers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                server_printer_id INTEGER,
                name TEXT NOT NULL,
                connection_type TEXT NOT NULL DEFAULT 'network',
                ip_address TEXT,
                port INTEGER DEFAULT 9100,
                paper_width INTEGER DEFAULT 80,
                is_online INTEGER NOT NULL DEFAULT 0,
                last_test_at TEXT,
                last_test_success INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX idx_printers_connection ON printers(connection_type);",
        ).unwrap();

        init_queue_db(&db).unwrap(); // مسار الهجرة يضيف العمودين

        let saved = save_printer_with(&db, sample());
        let got = PrintQueue::get_printer_by_server_id(&db, 7).unwrap();
        assert_eq!(got.hosted, Some(true)); // العمود أُضيف بالهجرة ثم حُفظت قيمته
        assert_eq!(got.device_path, None);
        assert_eq!(got.id, saved.id);
    }

    fn save_printer_with(db: &Connection, p: SavedPrinter) -> SavedPrinter {
        PrintQueue::save_printer(db, &p).unwrap()
    }
}
