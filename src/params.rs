use schemars::JsonSchema;
use serde::Deserialize;

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
