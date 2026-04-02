use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::db;

pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub batch_cancel: Arc<AtomicBool>,
}

impl AppState {
    pub fn new() -> Result<Self, String> {
        let conn = db::init_db().map_err(|e| e.to_string())?;
        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            batch_cancel: Arc::new(AtomicBool::new(false)),
        })
    }
}
