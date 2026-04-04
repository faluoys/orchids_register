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
    pub active_completion_accounts: Arc<Mutex<std::collections::HashSet<i64>>>,
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
            active_completion_accounts: Arc::new(Mutex::new(std::collections::HashSet::new())),
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

    pub fn try_register_completion_window(&self, account_id: i64) -> Result<bool, String> {
        let mut active = self
            .active_completion_accounts
            .lock()
            .map_err(|e| e.to_string())?;
        Ok(active.insert(account_id))
    }

    pub fn has_completion_window(&self, account_id: i64) -> Result<bool, String> {
        let active = self
            .active_completion_accounts
            .lock()
            .map_err(|e| e.to_string())?;
        Ok(active.contains(&account_id))
    }

    pub fn clear_completion_window(&self, account_id: i64) -> Result<bool, String> {
        let mut active = self
            .active_completion_accounts
            .lock()
            .map_err(|e| e.to_string())?;
        Ok(active.remove(&account_id))
    }
}

#[cfg(test)]
mod tests {
    use super::AppState;

    #[test]
    fn completion_window_registration_rejects_duplicates() {
        let state = AppState::new().expect("state should initialize");

        assert!(state.try_register_completion_window(42).unwrap());
        assert!(!state.try_register_completion_window(42).unwrap());
    }

    #[test]
    fn completion_window_registration_allows_reopen_after_clear() {
        let state = AppState::new().expect("state should initialize");

        assert!(state.try_register_completion_window(7).unwrap());
        state.clear_completion_window(7).unwrap();
        assert!(state.try_register_completion_window(7).unwrap());
    }
}
