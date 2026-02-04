use crate::app::types::AppConfig;
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;

// デフォルト値を include_str! で埋め込み
const DEFAULT_CONFIG_JSON: &str = include_str!("../../../src/assets/AppConfig.json");

pub fn default_config() -> AppConfig {
    serde_json::from_str(DEFAULT_CONFIG_JSON)
        .expect("Embedded AppConfig.json is invalid. Please check the JSON format at compile time.")
}

pub fn load_app_config(app: &AppHandle) -> Result<(AppConfig, PathBuf), String> {
    // ユーザーの設定ディレクトリを取得 (例: AppData/Roaming/cg-file-backup)
    let app_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;

    // ディレクトリがなければ作成 (MkdirAll 相当)
    if !app_dir.exists() {
        fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;
    }

    let config_path = app_dir.join("AppConfig.json");

    let data = if config_path.exists() {
        // 既存ファイルを読み込む
        fs::read_to_string(&config_path).map_err(|e| e.to_string())?
    } else {
        // なければデフォルトを書き込んで使う
        fs::write(&config_path, DEFAULT_CONFIG_JSON).map_err(|e| e.to_string())?;
        DEFAULT_CONFIG_JSON.to_string()
    };

    // デシリアライズ
    let cfg: AppConfig = serde_json::from_str(&data).map_err(|e| e.to_string())?;

    Ok((cfg, config_path))
}
