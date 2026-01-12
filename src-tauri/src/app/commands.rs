// 標準ライブラリ
use std::fs;
use std::path::{Path, PathBuf};

// 外部クレート
use chrono::{DateTime, Local};
use tauri::{AppHandle, LogicalSize, Manager, Size, State, WebviewWindow, Window};

// Tauriプラグイン
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_shell::ShellExt;

// 内部モジュール (自作)
use crate::app::hdiff_common::*;
use crate::app::state::AppState;
use crate::app::types::BackupItem;
use crate::app::types::*;
use crate::app::{auto_generation, utils};
use flate2::read::GzDecoder;
use std::collections::HashMap;
use std::fs::File;
use tar::Archive;
use zip::ZipArchive;

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    Ok(cfg.clone())
}

#[tauri::command]
pub fn set_always_on_top(
    window: Window,
    state: State<'_, AppState>,
    flag: bool,
) -> Result<(), String> {
    // 1. ウィンドウの設定変更
    window.set_always_on_top(flag).map_err(|e| e.to_string())?;

    // 2. 設定の保存
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.always_on_top = flag;
    }
    state.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_restore_previous_state(state: State<'_, AppState>) -> bool {
    state.config.lock().unwrap().restore_previous_state
}

#[tauri::command]
pub fn get_bsdiff_max_file_size(state: State<'_, AppState>) -> i64 {
    state.config.lock().unwrap().bsdiff_max_file_size
}

#[tauri::command]
pub fn get_auto_base_generation_threshold(state: State<'_, AppState>) -> f64 {
    state.config.lock().unwrap().auto_base_generation_threshold
}

/// 特定のキーに対応する翻訳テキストを返す (Goの GetLanguageText 相当)
/// Rust内部のメニュー構築などで使用する場合、AppStateを引数に取る形で実装

#[tauri::command]
pub fn get_language_text(state: State<'_, AppState>, key: &str) -> Result<String, String> {
    let cfg = state.config.lock().unwrap();
    let lang = if cfg.language.is_empty() {
        "ja"
    } else {
        &cfg.language
    };

    // i18n -> lang -> key を安全に辿る
    Ok(
        cfg.i18n
            .get(lang)
            .and_then(|dict| dict.get(key))
            .cloned()
            .unwrap_or_else(|| key.to_string()), // 見つからなければキー名をそのまま返す
    )
}

/// 現在の言語設定に基づいた辞書をまるごと返す (Goの GetI18N 相当)
#[tauri::command]
pub fn get_i18n(state: State<'_, AppState>) -> Result<HashMap<String, String>, String> {
    let cfg = state.config.lock().unwrap();
    let lang = if cfg.language.is_empty() {
        "ja"
    } else {
        &cfg.language
    };

    Ok(cfg.i18n.get(lang).cloned().unwrap_or_default())
}

/// 言語を切り替えて保存する (Goの SetLanguage 相当)
#[tauri::command]
pub fn set_language(state: State<'_, AppState>, lang: String) -> Result<(), String> {
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.language = lang;
    }
    // 前に作った state.save() を呼び出す
    state.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn backup_or_diff(
    app: AppHandle,
    work_file: String,
    custom_dir: String,
    algo: String,
    compress: String,
) -> Result<(), String> {
    use crate::app::state::AppState;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tauri::Manager;

    // --- 1. ディレクトリの決定 (Go版の root 変数に相当) ---
    // ここでの project_root は、常に baseN フォルダが作られる「親」を指すようにします。
    let initial_path = if custom_dir.is_empty() {
        utils::default_backup_dir(&work_file)
    } else {
        PathBuf::from(custom_dir)
    };

    let mut target_dir: PathBuf;
    let mut current_idx: i32 = 0;
    let project_root: PathBuf;

    let folder_name = initial_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    if folder_name.starts_with("base") {
        // A. 特定の世代フォルダ (.../baseN) が直接指定されている場合
        target_dir = initial_path.clone();
        project_root = initial_path.parent().unwrap_or(&initial_path).to_path_buf();

        // 正規表現でインデックス抽出 (Go版: Sscanf(baseFolder, "base%d", &currentIdx))
        if let Some(idx_str) = folder_name
            .strip_prefix("base")
            .and_then(|s| s.split('_').next())
        {
            current_idx = idx_str.parse().unwrap_or(0);
        }
    } else {
        // B. 親フォルダが指定されている場合 (Go版: a.ResolveGenerationDir)
        project_root = initial_path.clone();
        let (resolved_path, idx) =
            auto_generation::resolve_generation_dir(&project_root, &work_file)?;
        target_dir = resolved_path;
        current_idx = idx;
    }

    // フォルダの存在保証
    if !target_dir.exists() {
        fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    }

    let file_name = Path::new(&work_file)
        .file_name()
        .ok_or("Invalid work file name")?
        .to_string_lossy();
    let base_full = target_dir.join(format!("{}.base", file_name));

    // --- 2. .baseファイルの同期 ---
    if !base_full.exists() {
        fs::copy(&work_file, &base_full).map_err(|e| format!("Failed to sync base file: {}", e))?;
    }

    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let temp_diff = std::env::temp_dir().join(format!("{}.{}.tmp", file_name, ts));

    // 差分生成 (hdiff)
    if algo == "bsdiff" {
        return Err(String::from("`bsdiff` is not supported yet."));
    } else {
        crate::app::hdiff::create_hdiff(
            app.clone(),
            &base_full.to_string_lossy(),
            &work_file,
            &temp_diff.to_string_lossy(),
            &compress,
        )
        .await?;
    }

    // --- 3. サイズ・閾値判定 ---
    let work_size = fs::metadata(&work_file).map_err(|e| e.to_string())?.len();
    let diff_size = fs::metadata(&temp_diff).map_err(|e| e.to_string())?.len();

    // Configから閾値を動的に取得
    let threshold = {
        let state = app.state::<AppState>();
        let cfg = state.config.lock().unwrap();
        cfg.auto_base_generation_threshold // ここが 0.0001 なら即世代交代される
    };

    let mut should_next_gen = false;
    if work_size > 100 * 1024 {
        // 100KB超
        if (diff_size as f64) > (work_size as f64) * threshold {
            should_next_gen = true;
        }
    }

    if should_next_gen {
        // --- 4a. 【サイズ超過】 世代交代ロジック ---
        let _ = fs::remove_file(&temp_diff);
        let new_idx = current_idx + 1;

        // project_root（親）に対して新しい baseN を作る
        let new_gen_dir =
            auto_generation::create_new_generation(&project_root, new_idx, &work_file)?;

        let new_base_full = new_gen_dir.join(format!("{}.base", file_name));
        let final_path = new_gen_dir.join(format!("{}.{}.{}.diff", file_name, ts, algo));

        // 新しい .base に対して diff を作り直す
        crate::app::hdiff::create_hdiff(
            app.clone(),
            &new_base_full.to_string_lossy(),
            &work_file,
            &final_path.to_string_lossy(),
            &compress,
        )
        .await
    } else {
        // --- 4b. 【正常】 移動して確定 ---
        let final_path = target_dir.join(format!("{}.{}.{}.diff", file_name, ts, algo));
        fs::rename(&temp_diff, &final_path)
            .map_err(|e| format!("Failed to finalize diff: {}", e))?;
        Ok(())
    }
}

#[tauri::command]
pub async fn apply_multi_diff(
    app: AppHandle,
    work_file: String,
    diff_paths: Vec<String>,
) -> Result<(), String> {
    for dp in diff_paths {
        let diff_name = Path::new(&dp).file_name().unwrap().to_string_lossy();

        let result = if diff_name.contains(".bsdiff.") {
            return Err(String::from("`bsdiff` is not supported."));
        } else if diff_name.contains(".hdiff.") {
            apply_hdiff_wrapper(app.clone(), work_file.as_str(), dp.as_str()).await
        } else {
            // 古いファイルのリトライ戦略
            match apply_hdiff_wrapper(app.clone(), work_file.as_str(), dp.as_str()).await {
                Ok(_) => Ok(()),
                Err(e) => {
                    // bsdiffリトライの枠だけ
                    Err(format!("recovery failed for old format: {}", e))
                }
            }
        };

        if let Err(e) = result {
            return Err(format!("復元失敗 ({}): {}", diff_name, e));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn get_config_dir(app: AppHandle) -> String {
    // Tauriの組み込み機能で設定ディレクトリを取得
    // 取得に失敗した場合はフォールバックとして "./config" を返す
    let config_dir: PathBuf = match app.path().app_config_dir() {
        Ok(path) => path,
        Err(_) => return "./config".to_string(),
    };

    // フォルダが存在しない場合は作成 (MkdirAll 相当)
    if !config_dir.exists() {
        let _ = fs::create_dir_all(&config_dir);
    }

    // JS側には文字列として返す
    config_dir.to_string_lossy().into_owned()
}

#[tauri::command]
pub fn get_file_size(path: String) -> Result<i64, String> {
    if path.is_empty() {
        return Err("path is empty".to_string());
    }

    let p = Path::new(&path);

    // ファイルのメタデータを取得 (os.Stat 相当)
    let metadata = fs::metadata(p).map_err(|e| e.to_string())?;

    // ディレクトリの場合はエラーを返す
    if metadata.is_dir() {
        return Err("path is a directory".to_string());
    }

    // サイズを返す (i64にキャスト)
    Ok(metadata.len() as i64)
}

#[tauri::command]
pub async fn select_any_file(app: AppHandle, title: String) -> Result<Option<String>, String> {
    // メインウィンドウを取得して親にする
    let window = app
        .get_webview_window("main")
        .ok_or("Main window not found")?;

    // window.dialog() を使うことで親子関係を持たせ、最前面問題を解決
    let file_path = window
        .dialog()
        .file()
        .set_title(&title)
        .blocking_pick_file();

    match file_path {
        Some(path) => Ok(Some(path.to_string())),
        None => Ok(None),
    }
}

/// フォルダ選択ダイアログを表示する
#[tauri::command]
pub async fn select_backup_folder(app: AppHandle) -> Result<Option<String>, String> {
    // メインウィンドウを取得して親にする
    let window = app
        .get_webview_window("main")
        .ok_or("Main window not found")?;

    let folder_path = window
        .dialog()
        .file()
        .set_title("Folder Select")
        .blocking_pick_folder();

    match folder_path {
        Some(path) => Ok(Some(path.to_string())),
        None => Ok(None),
    }
}

#[tauri::command]
pub fn open_directory(app: tauri::AppHandle, path: String) -> Result<(), String> {
    // 1. パスの親ディレクトリ（フォルダ）を取得
    let target = std::path::Path::new(&path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));

    // シェル操作（フォルダを開く）は、ダイアログとは異なり
    // OS自体の別アプリ（Explorer/Finder）を起動するため app.shell() のままで問題ありません
    app.shell()
        .open(target.to_string_lossy().to_string(), None)
        .map_err(|e| e.to_string())?;

    Ok(())
}

// コマンド用ラッパー
#[tauri::command]
pub async fn toggle_compact_mode(window: WebviewWindow, is_compact: bool) -> Result<(), String> {
    utils::apply_compact_mode(&window, is_compact).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_window_visibility(app: AppHandle, show: bool) -> Result<(), String> {
    utils::apply_window_visibility(app, show)
}

#[tauri::command]
pub fn read_text_file(path: String) -> Result<String, String> {
    let p = std::path::Path::new(&path);

    // 1. ファイルが存在するかチェック
    if !p.exists() {
        // Go版と同様、存在しない場合はエラーにせず空文字を返す
        return Ok("".to_string());
    }

    // 2. ファイルを読み込む
    // fs::read_to_string は UTF-8 を想定しています
    match fs::read_to_string(p) {
        Ok(content) => Ok(content),
        Err(e) => {
            // 読み込みに失敗した場合（権限不足など）はエラーを返す
            Err(format!("Failed to read file: {}", e))
        }
    }
}

/// 指定されたパスに文字列を書き込む (Goの WriteTextFile 相当)
#[tauri::command]
pub fn write_text_file(path: String, content: String) -> Result<(), String> {
    let path_obj = Path::new(&path);

    // 親ディレクトリが存在しない場合は作成する (Go版より少し親切な設計)
    if let Some(parent) = path_obj.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
        }
    }

    // ファイル書き込み (0644相当はRustの標準的な挙動)
    fs::write(path_obj, content).map_err(|e| format!("Failed to write text file: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn get_backup_list(work_file: String, backup_dir: String) -> Result<Vec<BackupItem>, String> {
    let mut list = Vec::new();

    // --- 1. ルートディレクトリの決定 ---
    let root = if backup_dir.is_empty() {
        utils::default_backup_dir(&work_file)
    } else {
        PathBuf::from(&backup_dir)
    };

    if !root.exists() {
        return Ok(list);
    }

    // ファイル名（拡張子なし）を取得
    let file_path_obj = Path::new(&work_file);
    let base_name_only = file_path_obj
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    let valid_exts = vec![".diff", ".zip", ".tar.gz", ".tar", ".gz"];

    // 拡張子判定ヘルパー
    let is_valid_ext = |name: &str| -> bool {
        let n = name.to_lowercase();
        valid_exts.iter().any(|ext| n.ends_with(ext))
    };

    // --- 1. ルート直下のアーカイブをスキャン ---
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                continue;
            }

            let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

            if file_name.contains(base_name_only) && is_valid_ext(file_name) {
                // fs::metadata で確実に最新のサイズを取得
                if let Ok(metadata) = fs::metadata(&path) {
                    list.push(create_backup_item(file_name, &path, &metadata, 0));
                }
            }
        }
    }

    // --- 2. すべての世代フォルダ(base*)をスキャン ---
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let dir_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

            if dir_name.starts_with("base") {
                // 【修正】base1_2024... から "1" だけを取り出すロジック
                let gen_idx: i32 = dir_name
                    .strip_prefix("base")
                    .and_then(|s| s.split('_').next()) // アンダーバーで区切って最初の要素を取る
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);

                if let Ok(gen_entries) = fs::read_dir(&path) {
                    for gen_entry in gen_entries.flatten() {
                        let gen_path = gen_entry.path();
                        let f_name = gen_path.file_name().and_then(|s| s.to_str()).unwrap_or("");

                        // 除外条件
                        if gen_path.is_dir()
                            || f_name.ends_with(".base")
                            || f_name == "checksum.json"
                        {
                            continue;
                        }

                        if f_name.contains(base_name_only) && is_valid_ext(f_name) {
                            // 世代フォルダ内のファイルも fs::metadata でサイズを確定
                            if let Ok(metadata) = fs::metadata(&gen_path) {
                                list.push(create_backup_item(
                                    f_name, &gen_path, &metadata, gen_idx,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(list)
}

// ヘルパー関数: 拡張子チェック
fn is_valid_backup_ext(name: &str, exts: &[&str]) -> bool {
    exts.iter().any(|&ext| name.ends_with(ext))
}

// ヘルパー関数: アイテム生成 (日付フォーマット含む)
fn create_backup_item(name: &str, path: &Path, meta: &fs::Metadata, gen: i32) -> BackupItem {
    let modified: DateTime<Local> = meta
        .modified()
        .unwrap_or_else(|_| std::time::SystemTime::now())
        .into();
    BackupItem {
        file_name: name.to_string(),
        file_path: path.to_string_lossy().into_owned(),
        timestamp: modified.format("%Y-%m-%d %H:%M:%S").to_string(),
        file_size: meta.len() as i64,
        generation: gen,
    }
}

/// ファイルをそのままコピーしてバックアップする (Go版の CopyBackupFile 相当)
#[tauri::command]
pub fn copy_backup_file(src: String, backup_dir: String) -> Result<String, String> {
    // 1. バックアップ先ディレクトリの決定
    // backup_dir が空ならソースファイルに基づいたデフォルトディレクトリを作成
    let target_dir = if backup_dir.is_empty() {
        utils::default_backup_dir(&src)
    } else {
        PathBuf::from(backup_dir)
    };

    // 2. ディレクトリの作成 (MkdirAll 0755 相当)
    // utils::copy_file 内部でも作成していますが、Go版の構造に合わせここで明示的に作成
    if !target_dir.exists() {
        fs::create_dir_all(&target_dir)
            .map_err(|e| format!("バックアップ先フォルダの作成に失敗しました: {}", e))?;
    }

    // 3. タイムスタンプ付きファイル名の生成 (例: filename_20260111_120000.ext)
    let new_filename = utils::timestamped_name(&src);

    // 4. 保存先のフルパスを組み立て
    let dest_path = target_dir.join(new_filename);
    let dest_str = dest_path.to_string_lossy();

    // 5. utils::copy_file (Sync処理付き) を実行
    utils::copy_file(&src, &dest_str).map_err(|e| e.to_string())?;

    // 6. 成功したら保存先のパスを返す (JS側での表示用)
    Ok(dest_str.into_owned())
}

#[tauri::command]
pub async fn archive_backup_file(
    src: String,
    backup_dir: String,
    format: String,
    password: String,
) -> Result<String, String> {
    // 1. バックアップ先の決定
    let target_dir = if backup_dir.is_empty() {
        utils::default_backup_dir(&src)
    } else {
        std::path::PathBuf::from(backup_dir)
    };

    if !target_dir.exists() {
        fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    }

    // 2. フォーマットによる分岐
    if format == "zip" {
        utils::zip_backup_file(&src, &target_dir, &password).map_err(|e| e.to_string())?;
    } else {
        utils::tar_backup_file(&src, &target_dir).map_err(|e| e.to_string())?;
    }

    Ok("Archive created successfully".to_string())
}

/// 指定されたパスがディレクトリとして存在するか確認します (Go版の DirExists 相当)
#[tauri::command]
pub fn dir_exists(path: String) -> Result<bool, String> {
    let p = Path::new(&path);
    // exists() かつ is_dir() であることを1行で判定できます
    Ok(p.is_dir())
}

#[tauri::command]
pub async fn restore_backup(
    app: tauri::AppHandle,
    path: String,
    work_file: String,
) -> Result<(), String> {
    let lower_path = path.to_lowercase();

    // 1. 差分パッチ (.diff)
    if lower_path.ends_with(".diff") {
        return apply_multi_diff(app, work_file, vec![path]).await;
    }

    // 復元先のパスを「別名」として自動生成
    let restored_path = utils::auto_output_path(&work_file);

    // 2. ZIPアーカイブ
    if lower_path.ends_with(".zip") {
        let file = File::open(&path).map_err(|e| e.to_string())?;
        let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;
        if archive.len() > 0 {
            let mut file_in_zip = archive.by_index(0).map_err(|e| e.to_string())?;
            return utils::save_to_work_file(&mut file_in_zip, &restored_path);
        }
    }

    // 3. TARアーカイブ (.tar.gz)
    if lower_path.ends_with(".tar.gz") {
        let file = File::open(&path).map_err(|e| e.to_string())?;
        let tar_gz = GzDecoder::new(file);
        let mut archive = Archive::new(tar_gz);
        if let Some(Ok(mut entry)) = archive.entries().map_err(|e| e.to_string())?.next() {
            return utils::save_to_work_file(&mut entry, &restored_path);
        }
    }

    // 4. フルコピー (.clip / .psd 等)
    // 既存の utils::copy_file を使用
    utils::copy_file(&path, &restored_path)?;
    Ok(())
}
