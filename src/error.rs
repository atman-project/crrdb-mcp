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
    Sql {
        message: String,
        schema: Option<String>,
    },
    #[error(
        "`query` is for readonly. For write operations, use `commit_records`/`modify_record`/`delete_record`/`create_table`/`alter_table`: {message}"
    )]
    ReadonlyViolation { message: String },
    #[error("unique violation: {message}")]
    UniqueViolation { message: String },
    #[error("unknown table: {table}; consider using `create_table` to create it first")]
    UnknownTable { table: String },
    #[error(
        "missing required field: {message}; Check the schema by calling `get_schema` and fix the SQL statement"
    )]
    MissingRequired { message: String },
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
        if let rusqlite::Error::SqliteFailure(failure, message) = err {
            let message = message.unwrap_or_else(|| "empty SQLite error message".to_string());
            return match failure.extended_code {
                ffi::SQLITE_CONSTRAINT_PRIMARYKEY | ffi::SQLITE_CONSTRAINT_UNIQUE => {
                    Error::UniqueViolation { message }
                }
                ffi::SQLITE_CONSTRAINT_NOTNULL => Error::MissingRequired { message },
                _ => match failure.code {
                    ffi::ErrorCode::ReadOnly => Error::ReadonlyViolation { message },
                    _ => Error::Sql {
                        message,
                        schema: None,
                    },
                },
            };
        }
        Error::Sql {
            message: err.to_string(),
            schema: None,
        }
    }
}
