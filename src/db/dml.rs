use rusqlite::params_from_iter;

use crate::{
    db::{
        Db,
        outputs::{CommitRecordsOutput, DeleteRecordsOutput, UpdateRecordsOutput},
        system::SYSTEM_TABLES,
        value::json_value_to_db,
    },
    error::Error,
    params::Record,
};

impl Db {
    pub fn commit_records(&self, records: Vec<Record>) -> Result<CommitRecordsOutput, Error> {
        Self::check_commit_records(&records)?;

        let tx = self.rw.unchecked_transaction()?;
        let mut n_inserted = 0;
        for Record { table, values } in records {
            let mut cols = Vec::with_capacity(values.len());
            let mut placeholders = Vec::with_capacity(values.len());
            let mut vals = Vec::with_capacity(values.len());
            for (i, (col, value)) in values.into_iter().enumerate() {
                cols.push(format!("\"{col}\""));
                placeholders.push(format!("${}", i + 1));
                vals.push(json_value_to_db(value)?);
            }
            let sql = format!(
                "INSERT INTO \"{table}\" ({}) VALUES ({})",
                cols.join(", "),
                placeholders.join(", ")
            );
            tx.execute(&sql, params_from_iter(vals.iter()))?;
            n_inserted += 1;
        }
        tx.commit()?;
        Ok(CommitRecordsOutput { n_inserted })
    }

    fn check_commit_records(records: &[Record]) -> Result<(), Error> {
        if records.is_empty() {
            return Err(Error::MissingRequired {
                message: "No record specified for `commit_records`".into(),
            });
        }
        for record in records {
            if SYSTEM_TABLES.contains(&record.table.as_str()) {
                return Err(Error::SystemTableMutationNotAllowed {
                    table: record.table.clone(),
                });
            }
        }
        Ok(())
    }

    pub fn update_records(
        &self,
        table: &str,
        keys: serde_json::Map<String, serde_json::Value>,
        patch: serde_json::Map<String, serde_json::Value>,
    ) -> Result<UpdateRecordsOutput, Error> {
        Self::check_update_records(table, &keys, &patch)?;

        let n_patches = patch.len();
        let mut set_clause = Vec::with_capacity(patch.len());
        let mut values = Vec::with_capacity(patch.len());
        let mut criteria = Vec::with_capacity(keys.len());

        for (i, (col, value)) in patch.into_iter().enumerate() {
            set_clause.push(format!("\"{col}\" = ${}", i + 1));
            values.push(json_value_to_db(value)?);
        }
        let set_clause = set_clause.join(", ");

        for (i, (col, value)) in keys.into_iter().enumerate() {
            criteria.push(format!("\"{col}\" = ${}", i + 1 + n_patches));
            values.push(json_value_to_db(value)?);
        }
        let mut criteria = criteria.join(" AND ");
        if !criteria.is_empty() {
            criteria = format!("WHERE {criteria}");
        }

        let n_updated = self.rw.execute(
            &format!("UPDATE \"{table}\" SET {set_clause} {criteria}"),
            params_from_iter(values.iter()),
        )?;

        Ok(UpdateRecordsOutput { n_updated })
    }

    fn check_update_records(
        table: &str,
        keys: &serde_json::Map<String, serde_json::Value>,
        patch: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), Error> {
        if keys.is_empty() {
            return Err(Error::MissingRequired {
                message: "No `keys` specified for `update_records`. It's prohibited to update all rows in the table.".into(),
            });
        }
        if patch.is_empty() {
            return Err(Error::MissingRequired {
                message: "No `patch` specified for `update_records`".into(),
            });
        }
        if SYSTEM_TABLES.contains(&table) {
            return Err(Error::SystemTableMutationNotAllowed {
                table: table.into(),
            });
        }
        Ok(())
    }

    pub fn delete_records(
        &self,
        table: &str,
        keys: serde_json::Map<String, serde_json::Value>,
    ) -> Result<DeleteRecordsOutput, Error> {
        Self::check_delete_records(table, &keys)?;

        let mut criteria = Vec::with_capacity(keys.len());
        let mut values = Vec::with_capacity(keys.len());
        for (i, (col, value)) in keys.into_iter().enumerate() {
            criteria.push(format!("\"{col}\" = ${}", i + 1));
            values.push(json_value_to_db(value)?);
        }
        let mut criteria = criteria.join(" AND ");
        if !criteria.is_empty() {
            criteria = format!("WHERE {criteria}");
        }

        let n_deleted = self.rw.execute(
            &format!("DELETE FROM \"{table}\" {criteria}"),
            params_from_iter(values.iter()),
        )?;
        Ok(DeleteRecordsOutput { n_deleted })
    }

    fn check_delete_records(
        table: &str,
        keys: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), Error> {
        if keys.is_empty() {
            return Err(Error::MissingRequired {
                message: "No `keys` specified for `delete_records`. It's prohibited to delete all rows in the table.".into(),
            });
        }

        if SYSTEM_TABLES.contains(&table) {
            return Err(Error::SystemTableMutationNotAllowed {
                table: table.into(),
            });
        }
        Ok(())
    }
}
