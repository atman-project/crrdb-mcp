pub mod ddl;
mod dml;
pub mod outputs;
pub mod query;
pub mod schema;
pub mod system;
pub mod value;

use std::path::Path;

use rusqlite::{Connection, OpenFlags};

pub struct Db {
    /// The read-write connection used for write operations created by
    /// either the client or the server itself. Also, used for internal read
    /// operations that need to see uncommitted changes.
    rw: Connection,
    /// The read-only connection used for read operations created by the client.
    ro: Connection,
}

impl Db {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }

        // Open a read-write connection.
        let rw = Connection::open(path)?;
        // Enable WAL for better concurrency: to not block read-only conns
        rw.pragma_update(None, "journal_mode", "WAL")?;
        // Create system tables if they don't exist.
        rw.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS _ddl_log (
                seq        INTEGER PRIMARY KEY,
                tbl        TEXT NOT NULL,
                stmt       TEXT NOT NULL,
                reason     TEXT,
                applied_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS _schema_doc (
                tbl         TEXT NOT NULL,
                col         TEXT,   -- NULL for table-level description
                description TEXT NOT NULL,
                PRIMARY KEY (tbl, col)
            );
            "#,
        )?;

        // Open a read-only connection.
        let ro = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        // Disable write operations on this connection.
        ro.pragma_update(None, "query_only", "ON")?;

        Ok(Self { rw, ro })
    }
}
