use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")] // これで基本はキャメルケースになる
pub struct AppConfig {
    pub language: String,
    pub always_on_top: bool,
    pub restore_previous_state: bool,
    pub tray_mode: bool,
    pub auto_base_generation_threshold: f64,
    pub i18n: HashMap<String, HashMap<String, String>>,
    #[serde(skip_serializing, default)]
    pub compact_mode: bool,
    pub tray_backup_mode: String,
}

// JS側で確実に受け取るための構造体
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DiffFileInfo {
    pub file_name: String, // test-project.clip.2025...diff
    pub file_path: String, // フルパス
    pub timestamp: String, // 2025... 部分
    pub file_size: i64,
}

// 履歴リストに表示する各ファイルの情報を保持
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BackupItem {
    pub file_name: String,
    pub file_path: String,
    pub timestamp: String,
    pub file_size: i64,
    pub generation: i32, // 世代番号
}

// 世代管理を司る構造体 (JSに送らない場合は Serialize 不要ですが、一応付与)
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GenerationManager {
    pub backup_root: String, // cg_backup_元ファイル名/ のパス
    pub threshold: f64,      // ベース更新の閾値 (例: 0.8 = 80%)
}

// 現在の世代情報
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BackupGenInfo {
    pub dir_path: PathBuf,
    pub base_idx: i32,
}
