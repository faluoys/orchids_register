use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::errors::AppError;

pub fn save_result_json(path: &str, payload: &Value) -> Result<String, AppError> {
    let abs_path = std::fs::canonicalize(path).unwrap_or_else(|_| {
        let p = Path::new(path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(p)
        }
    });

    let existing = if abs_path.exists() {
        match fs::read_to_string(&abs_path) {
            Ok(content) => serde_json::from_str::<Value>(&content).ok(),
            Err(_) => None,
        }
    } else {
        None
    };

    let output_data = match existing {
        Some(Value::Array(arr)) => {
            let mut arr = arr;
            arr.push(payload.clone());
            Value::Array(arr)
        }
        Some(Value::Object(map)) if !map.is_empty() => Value::Array(vec![Value::Object(map), payload.clone()]),
        _ => Value::Array(vec![payload.clone()]),
    };

    let content = serde_json::to_string_pretty(&output_data).map_err(|e| AppError::Io(e.to_string()))?;
    fs::write(&abs_path, content).map_err(|e| AppError::Io(e.to_string()))?;

    Ok(abs_path.to_string_lossy().to_string())
}
