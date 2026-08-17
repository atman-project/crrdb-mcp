use base64::prelude::*;

use crate::error::Error;

pub fn db_value_to_json(value: rusqlite::types::ValueRef) -> serde_json::Value {
    match value {
        rusqlite::types::ValueRef::Null => serde_json::Value::Null,
        rusqlite::types::ValueRef::Integer(i) => serde_json::Value::from(i),
        rusqlite::types::ValueRef::Real(f) => serde_json::Value::from(f),
        rusqlite::types::ValueRef::Text(t) => {
            serde_json::Value::String(String::from_utf8_lossy(t).into_owned())
        }
        rusqlite::types::ValueRef::Blob(b) => serde_json::Value::String(BASE64_STANDARD.encode(b)),
    }
}

pub fn json_value_to_db(value: serde_json::Value) -> Result<rusqlite::types::Value, Error> {
    Ok(match value {
        serde_json::Value::Null => rusqlite::types::Value::Null,
        serde_json::Value::Bool(b) => rusqlite::types::Value::Integer(b as i64),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                rusqlite::types::Value::Integer(i)
            } else {
                rusqlite::types::Value::Real(n.as_f64().ok_or(Error::JsonToDbValueConversion {
                    value: n.to_string(),
                    message: "Cannot convert a JSON number to f64 for DB".into(),
                })?)
            }
        }
        serde_json::Value::String(s) => rusqlite::types::Value::Text(s),
        serde_json::Value::Array(arr) => rusqlite::types::Value::Text(
            serde_json::to_string(&arr).expect("JSON array must be serializable back to JSON"),
        ),
        serde_json::Value::Object(obj) => rusqlite::types::Value::Text(
            serde_json::to_string(&obj).expect("JSON object must be serializable back to JSON"),
        ),
    })
}

pub fn now() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
