use crate::app::types::DiffFileInfo;
use crate::app::utils;
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};

// 1. GetHdiffList の移植
pub fn get_hdiff_list(
    work_file: &str,
    custom_dir: Option<String>,
) -> Result<Vec<DiffFileInfo>, String> {
    // custom_dir がなければデフォルトパスを取得
    let target_dir = match custom_dir {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => utils::default_backup_dir(work_file),
    };

    if !target_dir.exists() {
        return Ok(vec![]);
    }

    let mut list = Vec::new();
    for entry in fs::read_dir(target_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        let file_name = entry.file_name().to_string_lossy().into_owned();

        // ディレクトリではなく、拡張子が .diff のものを抽出
        if path.is_file() && file_name.ends_with(".diff") {
            let ts = utils::extract_timestamp_from_backup(&file_name).unwrap_or_default();
            list.push(DiffFileInfo {
                file_name,
                file_path: path.to_string_lossy().into_owned(),
                timestamp: ts,
                file_size: metadata.len() as i64,
            });
        }
    }
    Ok(list)
}

// 2. BackupOrHdiff の移植
pub async fn backup_or_hdiff(
    app: tauri::AppHandle,
    work_file: &str,
    custom_dir: Option<String>,
    compress: String,
) -> Result<(), String> {
    let target_dir = match custom_dir {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => utils::default_backup_dir(work_file),
    };

    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

    let base_name = Path::new(work_file).file_name().unwrap().to_string_lossy();
    let base_full = target_dir.join(format!("{}.base", base_name));

    if !base_full.exists() {
        // baseがなければコピーして終了
        fs::copy(work_file, &base_full).map_err(|e| e.to_string())?;
        return Ok(());
    }

    // タイムスタンプ生成 (Goの 20060102_150405 相当)
    let ts = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let diff_path = target_dir.join(format!("{}.{}.diff", base_name, ts));

    // hdiffz（Sidecar）を呼び出す
    crate::app::hdiff::create_hdiff(
        app,
        &base_full.to_string_lossy(),
        work_file,
        &diff_path.to_string_lossy(),
        &compress,
    )
    .await
}

// 3. ApplyHdiffWrapper の移植
pub async fn apply_hdiff_wrapper(
    app: tauri::AppHandle,
    work_file: &str,
    diff_file: &str,
) -> Result<(), String> {
    let diff_path = Path::new(diff_file);
    let backup_dir = diff_path.parent().unwrap();

    // 文字列操作で .base 名を特定
    let file_name = diff_path.file_name().unwrap().to_string_lossy();
    let mut base_name = format!("{}.base", file_name.split(".20").next().unwrap());
    let mut base_full = backup_dir.join(&base_name);

    if !base_full.exists() {
        let work_base_name = format!(
            "{}.base",
            Path::new(work_file).file_name().unwrap().to_string_lossy()
        );
        base_full = backup_dir.join(work_base_name);
    }

    let out_path = utils::auto_output_path(work_file);

    // hpatchz (Sidecar) を呼び出す
    crate::app::hdiff::apply_hdiff(app, &base_full.to_string_lossy(), diff_file, &out_path).await
}
