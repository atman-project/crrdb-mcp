use std::collections::HashSet;

use serde::Serialize;

/// Limit the number of rows returned to the client, to not overwhelm the
/// client's context.
pub const HARD_LIMIT: usize = 2000;

pub struct QueryOutput {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub truncated: bool,
}

#[derive(Serialize)]
pub struct CreateTableOutput {
    pub table: String,
}

#[derive(Serialize)]
pub struct AlterTableOutput {
    pub table: String,
    pub columns: HashSet<String>,
}

#[derive(Serialize)]
pub struct CommitRecordsOutput {
    pub n_inserted: usize,
}

#[derive(Serialize)]
pub struct UpdateRecordsOutput {
    pub n_updated: usize,
}

#[derive(Serialize)]
pub struct DeleteRecordsOutput {
    pub n_deleted: usize,
}
