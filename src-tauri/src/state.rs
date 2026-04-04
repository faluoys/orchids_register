use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::db;
use crate::service_manager::ServiceManager;

pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub batch_cancel: Arc<AtomicBool>,
    pub close_prompt_open: Arc<AtomicBool>,
    pub exit_requested: Arc<AtomicBool>,
    pub services: Arc<Mutex<ServiceManager>>,
}

impl AppState {
    pub fn new() -> Result<Self, String> {
        let conn = db::init_db().map_err(|e| e.to_string())?;
        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            batch_cancel: Arc::new(AtomicBool::new(false)),
            close_prompt_open: Arc::new(AtomicBool::new(false)),
            exit_requested: Arc::new(AtomicBool::new(false)),
            services: Arc::new(Mutex::new(ServiceManager::default())),
        })
    }

    pub fn begin_close_prompt(&self) -> bool {
        if self.exit_requested.load(Ordering::SeqCst) {
            return false;
        }
        self.close_prompt_open
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn cancel_close_prompt(&self) {
        self.close_prompt_open.store(false, Ordering::SeqCst);
    }

    pub fn allow_exit(&self) {
        self.exit_requested.store(true, Ordering::SeqCst);
        self.close_prompt_open.store(false, Ordering::SeqCst);
    }

    pub fn should_allow_exit(&self) -> bool {
        self.exit_requested.load(Ordering::SeqCst)
    }
}
