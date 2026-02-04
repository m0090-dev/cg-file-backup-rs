use crate::app::types::AppConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub config_path: PathBuf,
}

impl AppState {
    pub fn save(&self) -> Result<(), String> {
        let cfg = self.config.lock().unwrap();
        let data = serde_json::to_string_pretty(&*cfg).map_err(|e| e.to_string())?;
        fs::write(&self.config_path, data).map_err(|e: std::io::Error| e.to_string())?;
        Ok(())
    }
}
