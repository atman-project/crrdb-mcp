// TODO: contain DDL
pub const SYSTEM_TABLES: [&str; 2] = ["_ddl_log", "_schema_doc"];

pub const SYSTEM_COLUMNS: [(&str, &str); 2] = [
    ("_raw", "An utterance that created this row"),
    ("_said_at", "Time of the utterance (ISO8601)"),
];
