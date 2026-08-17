use std::collections::{HashMap, HashSet};

use rusqlite::params;

use crate::{db::Db, error::Error};

pub struct SchemaDoc {
    pub table: Description,
    pub columns: HashMap<String, Description>,
}

pub type Description = String;

pub struct TableInfo {
    pub name: String,
    pub columns: HashMap<String, ColumnInfo>,
    pub primary_key: Vec<String>,
}

impl TableInfo {
    pub fn column(&self, name: &str) -> Option<&ColumnInfo> {
        self.columns.get(name)
    }

    pub fn has_primary_key(&self) -> bool {
        !self.primary_key.is_empty()
    }
}

pub struct ColumnInfo {
    pub declared_type_upper: String,
    pub notnull: bool,
}

impl Db {
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

    pub(crate) fn all_table_names(&self) -> Result<HashSet<String>, Error> {
        Ok(self
            .rw
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")?
            .query_map([], |row| row.get(0))
            .and_then(|names| names.collect())?)
    }

    pub(crate) fn user_table_names(&self) -> Result<Vec<String>, Error> {
        Ok(self
            .rw
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table'
                    AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\' AND name NOT LIKE '\\_%' ESCAPE '\\'
                    ORDER BY name",
            )?
            .query_map([], |row| row.get(0))
            .and_then(|names| names.collect())?)
    }

    pub(crate) fn ddl(&self, table: &str) -> Result<String, Error> {
        Ok(self.rw.query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
            params![table],
            |row| row.get(0),
        )?)
    }

    pub(crate) fn schema_doc(&self, table: &str) -> Result<SchemaDoc, Error> {
        let table_desc = self.rw.query_row(
            "SELECT description FROM _schema_doc WHERE tbl=?1 AND col IS NULL",
            params![table],
            |row| row.get(0),
        )?;

        let mut stmt = self
            .rw
            .prepare("SELECT col, description FROM _schema_doc WHERE tbl=?1 AND col IS NOT NULL")?;
        let col_descs = stmt
            .query_map(params![table], |row| Ok((row.get(0)?, row.get(1)?)))
            .and_then(|col_descs| col_descs.collect())?;

        Ok(SchemaDoc {
            table: table_desc,
            columns: col_descs,
        })
    }

    pub(crate) fn table_info(&self, table: &str) -> Result<TableInfo, Error> {
        let mut primary_key = Vec::new();
        let columns = self
            .rw
            .prepare("SELECT name, type, \"notnull\", pk FROM pragma_table_info(?1)")?
            .query_map(params![table], |row| {
                let col_name = row.get::<_, String>(0)?;
                let pk_order = row.get::<_, i64>(3)?;
                if pk_order >= 1 {
                    primary_key.push((pk_order, col_name.clone()));
                }
                Ok((
                    col_name.clone(),
                    ColumnInfo {
                        declared_type_upper: row.get::<_, String>(1)?.to_ascii_uppercase(),
                        notnull: row.get(2)?,
                    },
                ))
            })?
            .collect::<Result<HashMap<_, _>, _>>()?;

        primary_key.sort_by_key(|(order, _)| *order);

        Ok(TableInfo {
            name: table.to_string(),
            columns,
            primary_key: primary_key.into_iter().map(|(_, name)| name).collect(),
        })
    }
}
