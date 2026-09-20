use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLog {
    pub id: i64,
    pub level: String,
    pub category: String,
    pub message: String,
    pub details: Option<String>,
    pub created_at: String,
}

pub fn init_logs_db(db: &Connection) -> SqlResult<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS agent_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            level TEXT NOT NULL DEFAULT 'info',
            category TEXT NOT NULL DEFAULT 'general',
            message TEXT NOT NULL,
            details TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_al_level ON agent_logs(level);
        CREATE INDEX IF NOT EXISTS idx_al_category ON agent_logs(category);"
    )?;
    Ok(())
}

pub struct AgentLogger;

impl AgentLogger {
    pub fn log(db: &Connection, level: &str, category: &str, message: &str, details: Option<&str>) -> SqlResult<()> {
        db.execute(
            "INSERT INTO agent_logs (level, category, message, details) VALUES (?1, ?2, ?3, ?4)",
            params![level, category, message, details],
        )?;
        Ok(())
    }

    pub fn info(db: &Connection, category: &str, message: &str) -> SqlResult<()> {
        Self::log(db, "info", category, message, None)
    }

    pub fn error(db: &Connection, category: &str, message: &str, details: Option<&str>) -> SqlResult<()> {
        Self::log(db, "error", category, message, details)
    }

    pub fn warn(db: &Connection, category: &str, message: &str) -> SqlResult<()> {
        Self::log(db, "warn", category, message, None)
    }

    pub fn get_recent(db: &Connection, limit: u32) -> SqlResult<Vec<AgentLog>> {
        let mut stmt = db.prepare(
            "SELECT id, level, category, message, details, created_at
             FROM agent_logs ORDER BY created_at DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit], |row| Ok(AgentLog {
            id: row.get(0)?,
            level: row.get(1)?,
            category: row.get(2)?,
            message: row.get(3)?,
            details: row.get(4)?,
            created_at: row.get(5)?,
        }))?;
        rows.collect()
    }
}
