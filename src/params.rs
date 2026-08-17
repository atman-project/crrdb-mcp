use std::collections::HashMap;

use schemars::JsonSchema;
use serde::Deserialize;

use crate::db::schema::Description;

#[derive(Deserialize, JsonSchema)]
pub struct QueryParams {
    pub sql: String,
}

const DEFAULT_SAMPLE_ROWS_LIMIT: usize = 5;

#[derive(Deserialize, JsonSchema)]
pub struct SampleRowsParams {
    pub table: String,
    pub limit: Option<usize>,
}

impl SampleRowsParams {
    pub fn limit(&self) -> usize {
        self.limit.unwrap_or(DEFAULT_SAMPLE_ROWS_LIMIT)
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct CreateTableParams {
    pub sql: String,
    pub description: String,
    pub column_descriptions: HashMap<String, Description>,
    pub reason: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct AlterTableParams {
    pub table: String,
    pub sql: String,
    pub column_descriptions: HashMap<String, Description>,
    pub reason: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct CommitRecordsParams {
    pub records: Vec<Record>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Record {
    pub table: String,
    pub values: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateRecordsParams {
    pub table: String,
    pub keys: serde_json::Map<String, serde_json::Value>,
    pub patch: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize, JsonSchema)]
pub struct DeleteRecordsParams {
    pub table: String,
    pub keys: serde_json::Map<String, serde_json::Value>,
}
