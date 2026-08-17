use crate::{
    db::{
        Db,
        outputs::{HARD_LIMIT, QueryOutput},
        value::db_value_to_json,
    },
    error::Error,
};

impl Db {
    pub fn sample_rows(&self, table: &str, limit: usize) -> Result<QueryOutput, Error> {
        let limit = std::cmp::min(limit, HARD_LIMIT);
        self.query(&format!(
            "SELECT * FROM \"{table}\" ORDER BY _said_at DESC LIMIT {limit}"
        ))
    }

    pub fn query(&self, sql: &str) -> Result<QueryOutput, Error> {
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
