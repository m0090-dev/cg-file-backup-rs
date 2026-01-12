use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
//use crate::app::types::R;
use crate::app::types::BackupGenInfo;

/// 最新の baseN_... フォルダを特定する
/// Go版の FindLatestBaseDir / GetLatestGeneration とロジックを完全同期
pub fn get_latest_generation(root: &Path) -> Result<Option<BackupGenInfo>, String> {
    if !root.exists() {
        return Ok(None);
    }

    let entries = fs::read_dir(root).map_err(|e| e.to_string())?;
    // Go版の ^base(\d+)_ に合わせる。アンダースコア以降があるもののみ対象
    let re = Regex::new(r"^base(\d+)_").unwrap();

    let mut latest_idx = -1;
    let mut latest_dir_name: Option<String> = None;

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(caps) = re.captures(&name) {
                // indexを取得
                if let Ok(idx) = caps[1].parse::<i32>() {
                    // Go版の `if idx >= maxIdx` を再現。
                    // idxが同じなら、文字列比較（タイムスタンプが新しい方）を優先
                    if idx > latest_idx || (idx == latest_idx && latest_dir_name.as_ref().map_or(true, |n| &name >= n)) {
                        latest_idx = idx;
                        latest_dir_name = Some(name);
                    }
                }
            }
        }
    }

    match latest_dir_name {
        Some(name) => Ok(Some(BackupGenInfo {
            dir_path: root.join(name),
            base_idx: latest_idx,
        })),
        None => Ok(None),
    }
}

/// 最新の世代フォルダを取得（なければ作成）
/// Go版 ResolveGenerationDir と同じく、存在しない場合は index 1 で作成する
pub fn resolve_generation_dir(root: &Path, work_file: &str) -> Result<(PathBuf, i32), String> {
    match get_latest_generation(root)? {
        Some(info) => Ok((info.dir_path, info.base_idx)),
        None => {
            // 世代が一つもない場合は、インデックス 1 で新規作成
            let new_path = create_new_generation(root, 1, work_file)?;
            Ok((new_path, 1))
        }
    }
}

/// 新しい世代フォルダを作成し、.base をコピーする
/// Go版 CreateNewGeneration に相当
pub fn create_new_generation(root: &Path, idx: i32, work_file: &str) -> Result<PathBuf, String> {
    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let new_dir_name = format!("base{}_{}", idx, ts);
    let new_dir_path = root.join(new_dir_name);

    // フォルダ作成 (mkdir -p)
    fs::create_dir_all(&new_dir_path).map_err(|e| e.to_string())?;

    // .base ファイルのコピー先パス
    let file_path = Path::new(work_file);
    let file_name = file_path
        .file_name()
        .ok_or_else(|| "Invalid work file name".to_string())?
        .to_string_lossy();

    let base_path = new_dir_path.join(format!("{}.base", file_name));

    // 実ファイルのコピー (CopyFile相当)
    fs::copy(work_file, &base_path).map_err(|e| format!("Failed to copy base file: {}", e))?;

    Ok(new_dir_path)
}

/// 新しい世代に切り替えるべきか判定する
/// Go版 ShouldRotate と同じロジック
pub fn should_rotate(base_path: &Path, diff_path: &Path, threshold: f64) -> bool {
    let base_size = fs::metadata(base_path).map(|m| m.len()).unwrap_or(0);
    let diff_size = fs::metadata(diff_path).map(|m| m.len()).unwrap_or(0);

    if base_size == 0 {
        return false;
    }

    // 差分サイズがベースサイズの Threshold 倍を超えているか
    (diff_size as f64) > (base_size as f64) * threshold
}
