use std::{collections::HashMap, path::Path};

use base64::prelude::*;
use rusqlite::{Connection, OpenFlags, params};

use crate::error::Error;

pub struct Db {
    /// The read-write connection used for write operations created by
    /// either the client or the server itself. Also, used for internal read operations that need to see uncommitted changes.
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

    pub fn get_schema(&self) -> Result<Option<String>, Error> {
        let tables = self.user_table_names()?;
        if tables.is_empty() {
            return Ok(None);
        }

        let mut out = String::new();
        for table in tables {
            let ddl = self.ddl(&table)?;
            out.push_str(&format!("## {table}\n{ddl};\n"));

            if let Ok(schema_doc) = self.schema_doc(&table) {
                out.push_str(&format!("-- Table: {}\n", schema_doc.table));
                for (col, desc) in schema_doc.columns {
                    out.push_str(&format!("---- {col}: {desc}\n"));
                }
            }
            out.push('\n');
        }
        Ok(Some(out))
    }

    fn user_table_names(&self) -> Result<Vec<String>, Error> {
        let mut stmt = self.rw.prepare(
            "SELECT name FROM sqlite_master WHERE type='table'
                AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\' AND name NOT LIKE '\\_%' ESCAPE '\\'
                ORDER BY name",
        )?;
        let names = stmt
            .query_map([], |row| row.get(0))
            .and_then(|names| names.collect())?;
        Ok(names)
    }

    fn ddl(&self, table: &str) -> Result<String, Error> {
        Ok(self.rw.query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
            params![table],
            |row| row.get(0),
        )?)
    }

    fn schema_doc(&self, table: &str) -> Result<SchemaDoc, Error> {
        let table_desc = self.rw.query_row(
            "SELECT description FROM _schema_doc WHERE table=?1 AND col IS NULL",
            params![table],
            |row| row.get(0),
        )?;

        let mut stmt = self.rw.prepare(
            "SELECT col, description FROM _schema_doc WHERE table=?1 AND col IS NOT NULL",
        )?;
        let col_descs = stmt
            .query_map(params![table], |row| Ok((row.get(0)?, row.get(1)?)))
            .and_then(|col_descs| col_descs.collect())?;

        Ok(SchemaDoc {
            table: table_desc,
            columns: col_descs,
        })
    }

    pub fn sample_rows(&self, table: &str, limit: usize) -> Result<QueryOutput, Error> {
        let limit = std::cmp::min(limit, HARD_LIMIT);
        self.query(&format!("SELECT * FROM \"{table}\" LIMIT {limit}"))
    }

    fn query(&self, sql: &str) -> Result<QueryOutput, Error> {
        let mut stmt = self.ro.prepare(sql)?;

        let columns = stmt
            .column_names()
            .iter()
            .map(|name| name.to_string())
            .collect::<Vec<_>>();
        let mut rows_iter = stmt.query([])?;
        let mut rows: Vec<Vec<serde_json::Value>> = Vec::new();
        let mut truncated = false;
        loop {
            match rows_iter.next() {
                Ok(Some(row)) => {
                    if rows.len() >= HARD_LIMIT {
                        truncated = true;
                        break;
                    }
                    let mut json_row = Vec::with_capacity(columns.len());
                    for (i, _) in columns.iter().enumerate() {
                        json_row.push(db_value_to_json(row.get_ref(i)?));
                    }
                    rows.push(json_row);
                }
                Ok(None) => break,
                Err(e) => return Err(e.into()),
            }
        }

        Ok(QueryOutput {
            columns,
            rows,
            truncated,
        })
    }
}

/// Limit the number of rows returned to the client, to not overwhelm the
/// client's context.
const HARD_LIMIT: usize = 2000;

pub struct SchemaDoc {
    pub table: Description,
    pub columns: HashMap<String, Description>,
}

pub type Description = String;

pub struct QueryOutput {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub truncated: bool,
}

fn db_value_to_json(value: rusqlite::types::ValueRef) -> serde_json::Value {
    match value {
        rusqlite::types::ValueRef::Null => serde_json::Value::Null,
        rusqlite::types::ValueRef::Integer(i) => serde_json::Value::from(i),
        rusqlite::types::ValueRef::Real(f) => serde_json::Value::from(f),
        rusqlite::types::ValueRef::Text(t) => {
            serde_json::Value::String(String::from_utf8_lossy(t).into_owned())
        }
        rusqlite::types::ValueRef::Blob(b) => serde_json::Value::String(BASE64_STANDARD.encode(b)),
    }
}
