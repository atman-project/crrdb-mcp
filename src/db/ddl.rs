use std::collections::{HashMap, HashSet};

use rusqlite::{Transaction, params};

use crate::{
    db::{
        Db,
        outputs::{AlterTableOutput, CreateTableOutput},
        schema::{Description, TableInfo},
        system::{SYSTEM_COLUMNS, SYSTEM_TABLES},
        value::now,
    },
    error::Error,
};

impl Db {
    pub fn create_table(
        &self,
        sql: &str,
        desc: &str,
        column_descs: &HashMap<String, Description>,
        reason: Option<&String>,
    ) -> Result<CreateTableOutput, Error> {
        let sql = sql.trim();
        Self::precheck_create_table(sql, desc)?;

        // Phase 1: probe
        let table = self.probe_create_table(sql)?;
        Self::check_table(&table, column_descs)?;

        // Phase 2: In one tx, execute `sql` and add rows to `_schema_doc`/`_ddl_log`.
        let tx = self.rw.unchecked_transaction()?;
        tx.execute(sql, [])?;
        insert_table_schema_doc(&tx, &table.name, desc)?;
        for col in table.columns.keys() {
            let col_desc = column_descs.get(col).ok_or(Error::ColumnDescription {
                column: col.clone(),
                message: format!("Missing column description for {col}. Add it."),
            })?;
            insert_column_schema_doc(&tx, &table.name, col.as_str(), col_desc.as_str())?;
        }
        insert_ddl_log(&tx, &table.name, sql, reason.map(|reason| reason.as_str()))?;
        tx.commit()?;

        Ok(CreateTableOutput { table: table.name })
    }

    fn precheck_create_table(sql: &str, desc: &str) -> Result<(), Error> {
        if desc.trim().is_empty() {
            return Err(Error::MissingTableDescription);
        }

        let sql = sql.trim_start().to_ascii_lowercase();
        if !sql.starts_with("create table") {
            return Err(Error::Sql {
                message: "A `sql` for `create_table` must be a `CREATE TABLE` statement".into(),
            });
        }

        if sql.contains("autoincrement") {
            return Err(Error::Sql {
                message: "`AUTOINCREMENT` is prohibitted for conflict-free replications".into(),
            });
        }

        if !sql.contains("strict") {
            return Err(Error::Sql {
                message: "`STRICT` must be set to `CREATE TABLE` to enable the strict typing mode"
                    .into(),
            });
        }

        Ok(())
    }

    fn check_table(
        table: &TableInfo,
        column_descs: &HashMap<String, Description>,
    ) -> Result<(), Error> {
        Self::check_table_name(&table.name)?;

        if !table.has_primary_key() {
            return Err(Error::NoPrimaryKey {
                table: table.name.clone(),
            });
        }

        for (col, expected_col_desc) in SYSTEM_COLUMNS {
            let col_info = table.column(col).ok_or(Error::MissingRequired {
                message: format!(
                    "A system column `{col}` does not exist. Add `{col} TEXT NOT NULL`."
                ),
            })?;

            if col_info.declared_type_upper != "TEXT" {
                return Err(Error::Sql {
                    message: format!("A system column `{col}` must be defined as `TEXT`"),
                });
            }

            if !col_info.notnull {
                return Err(Error::Sql {
                    message: format!("A system column `{col}` must be defined as `NOT NULL`"),
                });
            }

            let col_desc = column_descs.get(col).ok_or(Error::ColumnDescription {
                column: col.into(),
                message: format!("Column description for {col} must be specified"),
            })?;
            if col_desc != expected_col_desc {
                return Err(Error::ColumnDescription {
                    column: col.into(),
                    message: format!(
                        "Column description for {col} must be exactly '{expected_col_desc}'"
                    ),
                });
            }
        }

        Ok(())
    }

    fn check_table_name(name: &str) -> Result<(), Error> {
        if name.starts_with('_') {
            return Err(Error::Sql {
                message: format!(
                    "Cannot create table {name}: Table name shouldn't start with '_' since it is only for system tables. Use another table name."
                ),
            });
        } else if name.to_ascii_lowercase().starts_with("sqlite_") {
            return Err(Error::Sql {
                message: format!(
                    "Cannot create table {name}: Table name shouldn't start with 'sqlite_' since it is only for SQLite system tables. Use another table name."
                ),
            });
        }
        Ok(())
    }

    /// Creates a table just to check the table name and PK, and rollbacks it.
    fn probe_create_table(&self, sql: &str) -> Result<TableInfo, Error> {
        self.rw.execute("SAVEPOINT _probe", [])?;
        let prev_table_names = self.all_table_names()?;
        let result = self
            .rw
            .execute(sql, [])
            .map_err(Error::from)
            .and_then(|_| {
                let new_table_names = self.new_table_names(&prev_table_names)?;
                match new_table_names.len() {
                    1 => Ok(new_table_names.into_iter().next().unwrap()),
                    0 => Err(Error::Sql {
                        message: "Failed to create table".into(),
                    }),
                    _ => Err(Error::Sql {
                        message: "Only one table can be created at a time".into(),
                    }),
                }
            })
            .and_then(|table| self.table_info(&table));
        self.rw
            .execute_batch("ROLLBACK TO _probe; RELEASE _probe")?;
        result
    }

    fn new_table_names(
        &self,
        prev_table_names: &HashSet<String>,
    ) -> Result<HashSet<String>, Error> {
        let cur_table_names = self.all_table_names()?;
        Ok(cur_table_names
            .into_iter()
            .filter(|name| !prev_table_names.contains(name))
            .collect())
    }

    pub fn alter_table(
        &self,
        table: &str,
        sql: &str,
        reason: &str,
        column_descs: &HashMap<String, Description>,
    ) -> Result<AlterTableOutput, Error> {
        Self::precheck_alter_table(table, sql, reason)?;

        let tx = self.rw.unchecked_transaction()?;
        tx.execute(sql, [])?;
        for (col, col_desc) in column_descs {
            insert_column_schema_doc(&tx, table, col.as_str(), col_desc.as_str())?;
        }
        insert_ddl_log(&tx, table, sql, Some(reason))?;
        tx.commit()?;

        Ok(AlterTableOutput {
            table: table.into(),
            columns: column_descs.keys().cloned().collect(),
        })
    }

    fn precheck_alter_table(table: &str, sql: &str, reason: &str) -> Result<(), Error> {
        if SYSTEM_TABLES.contains(&table) {
            return Err(Error::AlterSystemTableNotAllowed);
        }
        if reason.trim().is_empty() {
            return Err(Error::MissingAlterTableReason);
        }
        let sql = sql.trim_start().to_ascii_lowercase();
        if !sql.starts_with("alter table") {
            return Err(Error::Sql {
                message: "A `sql` for `alter_table` must be a `ALTER TABLE` statement".into(),
            });
        }
        if sql.contains("drop column") {
            return Err(Error::DropColumnNotAllowed);
        }
        if sql.contains("rename column") {
            return Err(Error::RenameColumnNotAllowed);
        }
        if sql.contains("rename") {
            return Err(Error::RenameTableNotAllowed);
        }
        Ok(())
    }
}

fn insert_table_schema_doc(
    tx: &Transaction<'_>,
    table: &str,
    description: &str,
) -> Result<(), Error> {
    tx.execute(
        "INSERT OR REPLACE INTO _schema_doc(tbl, col, description) VALUES (?1, NULL, ?2)",
        params![table, description],
    )?;
    Ok(())
}

fn insert_column_schema_doc(
    tx: &Transaction<'_>,
    table: &str,
    col: &str,
    description: &str,
) -> Result<(), Error> {
    tx.execute(
        "INSERT OR REPLACE INTO _schema_doc(tbl, col, description) VALUES (?1, ?2, ?3)",
        params![table, col, description],
    )?;
    Ok(())
}

fn insert_ddl_log(
    tx: &Transaction<'_>,
    table: &str,
    sql: &str,
    reason: Option<&str>,
) -> Result<(), Error> {
    tx.execute(
        "INSERT INTO _ddl_log(tbl, stmt, reason, applied_at) VALUES (?1, ?2, ?3, ?4)",
        params![table, sql, reason, now()],
    )?;
    Ok(())
}
