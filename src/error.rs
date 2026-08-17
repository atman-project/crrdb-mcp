use rusqlite::ffi;
use serde::Serialize;

/// A structured error that contains detailed information,
/// so that clients can make informed decisions about how to handle the error.
#[derive(Debug, Serialize, thiserror::Error)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum Error {
    #[error("failed to acquire DB lock")]
    DbLock,
    #[error(
        "SQL error: {message}; Check the schema by calling `get_schema` and fix the SQL statement"
    )]
    Sql { message: String },
    #[error(
        "`query` is for readonly. For write operations, use `commit_records`/`update_record`/`delete_record`/`create_table`/`alter_table`: {message}"
    )]
    ReadonlyViolation { message: String },
    #[error(
        "unique violation: {message}; Check the schema by calling `get_schema` and fix the SQL statement by generating a new unique value for each unique column."
    )]
    UniqueViolation { message: String },
    #[error(
        "missing required field: {message}; Check the schema by calling `get_schema` and fix the SQL statement"
    )]
    MissingRequired { message: String },
    #[error("only a single statement must be specified")]
    MultipleStatement,
    #[error("no primary key defined for a table: {table}")]
    NoPrimaryKey { table: String },
    #[error(
        "table description must be specified; explain what this table is for in natural language."
    )]
    MissingTableDescription,
    #[error("column description error: {message}")]
    ColumnDescription { column: String, message: String },
    #[error(
        "the reason for `ALTER TABLE` must be specified; explain the reason in nature language."
    )]
    MissingAlterTableReason,
    #[error("`DROP COLUMN` is not allowed for conflict-free replications")]
    DropColumnNotAllowed,
    #[error("`RENAME COLUMN` is not allowed for conflict-free replications")]
    RenameColumnNotAllowed,
    #[error("Renaming table is not allowed for conflict-free replications")]
    RenameTableNotAllowed,
    #[error("altering the system tables is never allowed")]
    AlterSystemTableNotAllowed,
    #[error("mutating system tables is never allowed")]
    SystemTableMutationNotAllowed { table: String },
    #[error("cannot convert a JSON value to a DB value: {message}")]
    JsonToDbValueConversion { value: String, message: String },
}

impl Error {
    pub fn to_json(&self) -> String {
        let mut json = serde_json::to_value(self).unwrap_or_default();
        json["message"] = self.to_string().into();
        json.to_string()
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        match err {
            rusqlite::Error::SqliteFailure(failure, message) => {
                let message = message.unwrap_or_else(|| "empty SQLite error message".to_string());
                match failure.extended_code {
                    ffi::SQLITE_CONSTRAINT_PRIMARYKEY | ffi::SQLITE_CONSTRAINT_UNIQUE => {
                        Error::UniqueViolation { message }
                    }
                    ffi::SQLITE_CONSTRAINT_NOTNULL => Error::MissingRequired { message },
                    _ => match failure.code {
                        ffi::ErrorCode::ReadOnly => Error::ReadonlyViolation { message },
                        _ => Error::Sql { message },
                    },
                }
            }
            rusqlite::Error::MultipleStatement => Error::MultipleStatement,
            _ => Error::Sql {
                message: err.to_string(),
            },
        }
    }
}
