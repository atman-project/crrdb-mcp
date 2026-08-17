use std::sync::{Arc, Mutex};

use rmcp::{
    ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo},
    serde_json::json,
    tool, tool_handler, tool_router,
};

use crate::{
    db::{Db, outputs::QueryOutput},
    error::Error,
    params::{
        AlterTableParams, CommitRecordsParams, CreateTableParams, DeleteRecordsParams, QueryParams,
        SampleRowsParams, UpdateRecordsParams,
    },
};

const INSTRUCTIONS: &str = r#"crrdb-mcp: a local database that can be accessible by
AI agents as well as traditional DB tools.

Conventions:
- Call get_schema first when touching the DB for the first time in a session.
- If there is no table that the client wants to use, call create_table.
- When a value doesn't fit an existing table, consider alter_table (ADD COLUMN) before
creating a new table.
- The client is responsible for generating every row's primary key. If a random unique
value must be generated, generate a 26-char ULID (e.g., 01JGXK4YV0Q8Z3M9T2R5W7B1CD).
AUTOINCREMENT is forbidden since crrdb supports conflict-free replication.
- Put the user's utterance into `_raw` column verbatim — never a summary. `_said_at`
column is the time of utterance, ISO8601.
- When one utterance produces multiple rows, send them all in a single commit_records
call (atomic).
- Column names in human language (English snake_case); enum-like values as text, not
integer codes; dates as ISO8601 text.
- All arithmetic (sums, averages, returns) is done by SQL via query. Never compute
numbers yourself.
- Check the size with count before any large dump.
- Errors come back as JSON. Read kind/hint, fix the request, and retry once"#;

pub struct Server {
    db: Arc<Mutex<Db>>,
}

#[tool_handler]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("crrdb-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

#[tool_router]
impl Server {
    #[tool(
        description = "Return the full DDL plus natural-language descriptions of every \
        table and column. Call this first when touching the DB for the first time in a session."
    )]
    fn get_schema(&self) -> CallToolResult {
        tool_result(self.execute(|db| db.get_schema()).map(|maybe_schema| {
            match maybe_schema {
                Some(schema) => json!({"schema": schema}).to_string(),
                None => {
                    json!({"schema": "", "hint": "No table exists yet. Start with `create_table`"})
                        .to_string()
                }
            }
        }))
    }

    #[tool(description = "Return N rows (default 5) from a table. Use this \
        to get a feel for actual values — units, formatting conventions")]
    fn sample_rows(&self, Parameters(params): Parameters<SampleRowsParams>) -> CallToolResult {
        tool_result(
            self.execute(|db| db.sample_rows(&params.table, params.limit()))
                .map(|output| query_json(output).to_string()),
        )
    }

    #[tool(
        description = "Run a SQL query. Returns JSON with rows, count, and truncated flag. \
        If truncated, use aggregations or filters to reduce the result set. \
        Before any large query, first run a `SELECT count(*)` to check the table size."
    )]
    fn query(&self, Parameters(params): Parameters<QueryParams>) -> CallToolResult {
        tool_result(
            self.execute(|db| db.query(&params.sql))
                .map(|output| query_json(output).to_string()),
        )
    }

    #[tool(
        description = "Create a new table. Specify `description` to explain what this table is for. Also, specify the description of each column. Plus, specify `reason` why this table should be created. \
        Always add `STRICT` to enable the strict typing mode. \
        The table must have primary key, and system columns (`_raw` and `_said_at`). \
        The primary key can be a composite key. If there is no column to choose as primary key, \
        add the `id` column as `TEXT PRIMARY KEY`, which is a 26-char ULID. \
        The system columns must be `TEXT NOT NULL`. \
        The description of `_raw` must be 'An utterance that created this row'. \
        The description of `_said_at` must be 'Time of the utterance (ISO8601)'."
    )]
    fn create_table(&self, Parameters(params): Parameters<CreateTableParams>) -> CallToolResult {
        tool_result(
            self.execute(|db| {
                db.create_table(
                    &params.sql,
                    &params.description,
                    &params.column_descriptions,
                    params.reason.as_ref(),
                )
            })
            .map(|output| {
                serde_json::to_string(&output)
                    .expect("CreateTableOutput must be serializable to JSON")
            }),
        )
    }

    #[tool(
        description = "Alter a table. The client must specify the `reason` why this table should be altered. \
        The `table` must match with the table name in the `sql`. \
        It is recommened to use `ADD COLUMN` (NULL is allowed). `DROP COLUMN` and `RENAME COLUMN` are not allowed \
        for conflict-free replications."
    )]
    fn alter_table(&self, Parameters(params): Parameters<AlterTableParams>) -> CallToolResult {
        tool_result(
            self.execute(|db| {
                db.alter_table(
                    &params.table,
                    &params.sql,
                    &params.reason,
                    &params.column_descriptions,
                )
            })
            .map(|output| {
                serde_json::to_string(&output)
                    .expect("AlterTableOutput must be serializable to JSON")
            }),
        )
    }

    #[tool(
        description = "Commit records that an utterance requests, in one transaction. \
        Each row must have unique primary key values. If not, this tool will be failed."
    )]
    fn commit_records(
        &self,
        Parameters(params): Parameters<CommitRecordsParams>,
    ) -> CallToolResult {
        tool_result(
            self.execute(|db| db.commit_records(params.records))
                .map(|output| {
                    serde_json::to_string(&output)
                        .expect("CommitRecordsOutput must be serializable to JSON")
                }),
        )
    }

    #[tool(
        description = "Update records in a table. Specify only columns that should be updated. \
        Be careful to not update records that the user doesn't want"
    )]
    fn update_records(
        &self,
        Parameters(params): Parameters<UpdateRecordsParams>,
    ) -> CallToolResult {
        tool_result(
            self.execute(|db| db.update_records(&params.table, params.keys, params.patch))
                .map(|output| {
                    serde_json::to_string(&output)
                        .expect("UpdateRecordsOutput must be serializable to JSON")
                }),
        )
    }

    #[tool(
        description = "Remove records from a table. Call this only when the user explicitly wants. \
        Be careful to not remove records that the user doesn't want"
    )]
    fn delete_records(
        &self,
        Parameters(params): Parameters<DeleteRecordsParams>,
    ) -> CallToolResult {
        tool_result(
            self.execute(|db| db.delete_records(&params.table, params.keys))
                .map(|output| {
                    serde_json::to_string(&output)
                        .expect("DeleteRecordsOutput must be serializable to JSON")
                }),
        )
    }
}

impl Server {
    pub fn new(db: Db) -> Self {
        Self {
            db: Arc::new(Mutex::new(db)),
        }
    }

    fn execute<Output>(
        &self,
        func: impl FnOnce(&Db) -> Result<Output, Error>,
    ) -> Result<Output, Error> {
        let lock = match self.db.lock() {
            Ok(lock) => lock,
            Err(_) => {
                return Err(Error::DbLock);
            }
        };
        func(&lock)
    }
}

fn tool_result(result: Result<String, Error>) -> CallToolResult {
    match result {
        Ok(json) => CallToolResult::success(vec![ContentBlock::text(json)]),
        Err(e) => CallToolResult::error(vec![ContentBlock::text(e.to_json())]),
    }
}

fn query_json(output: QueryOutput) -> serde_json::Value {
    let rows = output
        .rows
        .iter()
        .map(|row| {
            serde_json::Value::Object(
                output
                    .columns
                    .iter()
                    .cloned()
                    .zip(row.iter().cloned())
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    if output.truncated {
        json!({
            "rows": rows,
            "count": rows.len(),
            "truncated": true,
            "notice": format!("The result was truncated, and only {} rows returned. Use aggregations or filters", rows.len()),
        })
    } else {
        json!({
            "rows": rows,
            "count": rows.len(),
            "truncated": false,
        })
    }
}
