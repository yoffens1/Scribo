use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use rusqlite::{params_from_iter, Connection};
use serde_json::Value;

pub struct DbState(pub Mutex<Option<Connection>>);

fn json_to_rusqlite(val: Value) -> Result<rusqlite::types::Value, String> {
    match val {
        Value::Null => Ok(rusqlite::types::Value::Null),
        Value::Bool(b) => Ok(rusqlite::types::Value::Integer(if b { 1 } else { 0 })),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(rusqlite::types::Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(rusqlite::types::Value::Real(f))
            } else {
                Err("Invalid number format".to_string())
            }
        }
        Value::String(s) => Ok(rusqlite::types::Value::Text(s)),
        Value::Array(arr) => {
            let mut bytes = Vec::with_capacity(arr.len());
            for v in arr {
                if let Some(n) = v.as_u64() {
                    if n <= 255 {
                        bytes.push(n as u8);
                    } else {
                        return Err(format!("Array element {} exceeds u8 limit", n));
                    }
                } else if let Some(i) = v.as_i64() {
                    if i >= 0 && i <= 255 {
                        bytes.push(i as u8);
                    } else {
                        return Err(format!("Array element {} is out of u8 range", i));
                    }
                } else {
                    return Err("Array contains non-integer elements for BLOB conversion".to_string());
                }
            }
            Ok(rusqlite::types::Value::Blob(bytes))
        }
        Value::Object(_) => Err("Object parameters are not supported in SQLite".to_string()),
    }
}

// FS Commands
#[tauri::command]
fn fs_read_text(path: String) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| e.to_string())
}

#[tauri::command]
fn fs_read_binary(path: String) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|e| e.to_string())
}

#[tauri::command]
fn fs_write_text(path: String, content: String) -> Result<(), String> {
    fs::write(path, content).map_err(|e| e.to_string())
}

#[tauri::command]
fn fs_exists(path: String) -> bool {
    PathBuf::from(path).exists()
}

#[derive(serde::Serialize)]
struct FsEntry {
    name: String,
    is_dir: bool,
}

#[tauri::command]
fn fs_list(path: String) -> Result<Vec<FsEntry>, String> {
    let entries = fs::read_dir(path).map_err(|e| e.to_string())?;
    let mut res = Vec::new();
    for entry in entries {
        if let Ok(entry) = entry {
            res.push(FsEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                is_dir: entry.file_type().map(|t| t.is_dir()).unwrap_or(false),
            });
        }
    }
    Ok(res)
}

#[tauri::command]
fn fs_rename(from: String, to: String) -> Result<(), String> {
    fs::rename(from, to).map_err(|e| e.to_string())
}

#[tauri::command]
fn fs_delete(path: String) -> Result<(), String> {
    let p = PathBuf::from(path);
    if p.is_dir() {
        fs::remove_dir_all(p).map_err(|e| e.to_string())
    } else {
        fs::remove_file(p).map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn db_initialize(state: tauri::State<'_, DbState>, db_path: String) -> Result<(), String> {
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA foreign_keys = ON;").map_err(|e| e.to_string())?;
    let mut db_guard = state.0.lock().map_err(|e| e.to_string())?;
    *db_guard = Some(conn);
    Ok(())
}

#[tauri::command]
fn db_close(state: tauri::State<'_, DbState>) -> Result<(), String> {
    let mut db_guard = state.0.lock().map_err(|e| e.to_string())?;
    *db_guard = None;
    Ok(())
}

#[tauri::command]
fn db_execute(
    state: tauri::State<'_, DbState>,
    query: String,
    params: Vec<Value>,
) -> Result<usize, String> {
    let mut opt_conn = state.0.lock().map_err(|e| e.to_string())?;
    let conn = opt_conn.as_mut().ok_or("Database not initialized")?;

    if query.trim().to_uppercase().starts_with("BEGIN") || 
       query.trim().to_uppercase().starts_with("COMMIT") || 
       query.trim().to_uppercase().starts_with("ROLLBACK") {
        conn.execute_batch(&query).map_err(|e| e.to_string())?;
        return Ok(0);
    }

    let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;
    let rusqlite_params: Vec<rusqlite::types::Value> = params
        .into_iter()
        .map(json_to_rusqlite)
        .collect::<Result<_, _>>()?;

    stmt.execute(params_from_iter(rusqlite_params)).map_err(|e| e.to_string())
}

#[tauri::command]
fn db_select(
    state: tauri::State<'_, DbState>,
    query: String,
    params: Vec<Value>,
) -> Result<Value, String> {
    let mut opt_conn = state.0.lock().map_err(|e| e.to_string())?;
    let conn = opt_conn.as_mut().ok_or("Database not initialized")?;

    let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;
    
    let rusqlite_params: Vec<rusqlite::types::Value> = params
        .into_iter()
        .map(json_to_rusqlite)
        .collect::<Result<_, _>>()?;

    let col_count = stmt.column_count();
    let mut columns = Vec::with_capacity(col_count);
    for i in 0..col_count {
        columns.push(stmt.column_name(i).map_err(|e| e.to_string())?.to_string());
    }

    let mut rows_iter = stmt
        .query(params_from_iter(rusqlite_params))
        .map_err(|e| e.to_string())?;

    let mut values = Vec::new();
    while let Some(row) = rows_iter.next().map_err(|e| e.to_string())? {
        let mut row_values = Vec::with_capacity(col_count);
        for i in 0..col_count {
            let val = row.get_ref(i).map_err(|e| e.to_string())?;
            let json_val = match val {
                rusqlite::types::ValueRef::Null => Value::Null,
                rusqlite::types::ValueRef::Integer(i) => Value::Number(i.into()),
                rusqlite::types::ValueRef::Real(f) => {
                    if let Some(n) = serde_json::Number::from_f64(f) {
                        Value::Number(n)
                    } else {
                        Value::Null
                    }
                }
                rusqlite::types::ValueRef::Text(t) => {
                    let s = std::str::from_utf8(t).map_err(|e| e.to_string())?;
                    Value::String(s.to_string())
                }
                rusqlite::types::ValueRef::Blob(b) => {
                    let arr = b.iter().map(|&x| Value::Number(x.into())).collect();
                    Value::Array(arr)
                }
            };
            row_values.push(json_val);
        }
        values.push(Value::Array(row_values));
    }

    let result = serde_json::json!({
        "columns": columns,
        "values": values
    });
    
    Ok(Value::Array(vec![result]))
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(DbState(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            greet,
            fs_read_text,
            fs_read_binary,
            fs_write_text,
            fs_exists,
            fs_list,
            fs_rename,
            fs_delete,
            db_initialize,
            db_close,
            db_execute,
            db_select
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
