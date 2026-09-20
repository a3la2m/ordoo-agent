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
            is_online INTEGER NOT NULL DEFAULT 0,
            last_test_at TEXT,
            last_test_success INTEGER,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_printers_connection ON printers(connection_type);"
    )?;
    Ok(())
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
                "INSERT INTO printers (server_printer_id, name, connection_type, ip_address, port, paper_width)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![p.server_printer_id, p.name, p.connection_type, p.ip_address, p.port, p.paper_width],
            )?;
            let id = db.last_insert_rowid();
            Ok(SavedPrinter { id, ..p.clone() })
        } else {
            db.execute(
                "UPDATE printers SET name=?1, connection_type=?2, ip_address=?3, port=?4, paper_width=?5 WHERE id=?6",
                params![p.name, p.connection_type, p.ip_address, p.port, p.paper_width, p.id],
            )?;
            Ok(p.clone())
        }
    }

    pub fn get_printers(db: &Connection) -> SqlResult<Vec<SavedPrinter>> {
        let mut stmt = db.prepare(
            "SELECT id, server_printer_id, name, connection_type, ip_address, port, paper_width, is_online, last_test_at, last_test_success
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
            is_online: row.get::<_, i32>(7)? != 0,
            last_test_at: row.get(8)?,
            last_test_success: row.get::<_, Option<i32>>(9)?.map(|v| v != 0),
        }))?;
        rows.collect()
    }

    pub fn delete_printer(db: &Connection, id: i64) -> SqlResult<()> {
        db.execute("DELETE FROM printers WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn get_printer_by_id(db: &Connection, id: i64) -> SqlResult<SavedPrinter> {
        db.query_row(
            "SELECT id, server_printer_id, name, connection_type, ip_address, port, paper_width, is_online, last_test_at, last_test_success
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
                is_online: row.get::<_, i32>(7)? != 0,
                last_test_at: row.get(8)?,
                last_test_success: row.get::<_, Option<i32>>(9)?.map(|v| v != 0),
            }),
        )
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
    pub is_online: bool,
    pub last_test_at: Option<String>,
    pub last_test_success: Option<bool>,
}
