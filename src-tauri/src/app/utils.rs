use crate::app::commands::get_language_text;
use crate::app::state::AppState;
use crate::app::types::AppConfig;
use chrono::Local;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use tar::Archive;
use tar::Builder;
use tauri::WebviewWindow;
use tauri::{AppHandle, Manager};
use tauri::{LogicalSize, Size, Window};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_shell::ShellExt;
use zip::write::SimpleFileOptions;
use zip::ZipArchive;
use zip::ZipWriter;
use zip::{AesMode, CompressionMethod};

/// ファイル名からタイムスタンプを抽出する (Go版のロジック通り)
pub fn extract_timestamp_from_backup(path: &str) -> Result<String, String> {
    let base = Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();

    let parts: Vec<&str> = base.split('.').collect();

    // test.clip.20251231_150000.diff -> 20251231_150000
    if parts.len() >= 3 {
        Ok(parts[parts.len() - 2].to_string())
    } else {
        Ok("No Timestamp".to_string())
    }
}

pub fn timestamped_name(original: &str) -> String {
    let path = Path::new(original);

    // 拡張子を除いたファイル名 (test.clip -> test)
    let file_stem = path
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();

    // 拡張子 (test.clip -> clip)
    let extension = path
        .extension()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();

    // 現在時刻をフォーマット
    let ts = Local::now().format("%Y%m%d_%H%M%S").to_string();

    // 拡張子がある場合とない場合で結合を分ける
    if extension.is_empty() {
        format!("{}_{}", file_stem, ts)
    } else {
        format!("{}_{}.{}", file_stem, ts, extension)
    }
}

pub fn auto_output_path(work_file: &str) -> String {
    let path = Path::new(work_file);
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let file_stem = path
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let ts = Local::now().format("%Y%m%d_%H%M%S").to_string();

    let new_filename = if extension.is_empty() {
        format!("{}_restored_{}", file_stem, ts)
    } else {
        format!("{}_restored_{}.{}", file_stem, ts, extension)
    };

    dir.join(new_filename).to_string_lossy().into_owned()
}

/// デフォルトのバックアップディレクトリを返す
pub fn default_backup_dir(work_file: &str) -> PathBuf {
    let path = Path::new(work_file);
    let dir = path.parent().unwrap_or_else(|| Path::new("."));

    let file_stem = path
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();

    // cg_backup_ファイル名 フォルダ
    dir.join(format!("cg_backup_{}", file_stem))
}

/// 単純なファイルコピーを行う (Go版の CopyFile 相当)
/// 親ディレクトリの作成、ストリームコピー、ディスク同期(Sync)を網羅
pub fn copy_file(src: &str, dst: &str) -> Result<(), String> {
    let src_path = Path::new(src);
    let dst_path = Path::new(dst);

    // 1. 出力先の親ディレクトリを MkdirAll (0755)
    if let Some(parent) = dst_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("ディレクトリ作成失敗: {}", e))?;
        }
    }

    // 2. 入力ファイルを開く (os.Open)
    let mut reader =
        File::open(src_path).map_err(|e| format!("入力ファイルが開けません {}: {}", src, e))?;

    // 3. 出力ファイルを作成 (os.Create)
    let mut writer = File::create(dst_path)
        .map_err(|e| format!("出力ファイルが作成できません {}: {}", dst, e))?;

    // 4. 内容をコピー (io.Copy)
    io::copy(&mut reader, &mut writer)
        .map_err(|e| format!("コピー中にエラーが発生しました: {}", e))?;

    // 5. ディスクに書き込みを確定させる (out.Sync)
    writer
        .sync_all()
        .map_err(|e| format!("ディスク同期に失敗しました: {}", e))?;

    Ok(())
}

pub fn zip_backup_file(src: &str, backup_dir: &Path, password: &str) -> Result<(), String> {
    // 1. 保存先の決定 (既存ロジック維持)
    let stem = Path::new(src)
        .file_stem()
        .ok_or("Invalid source path")?
        .to_string_lossy();
    let zip_filename = timestamped_name(&format!("{}.zip", stem));
    let zip_path = backup_dir.join(zip_filename);

    let file = File::create(&zip_path).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);

    // 2. オプション構築 (パスワードとAES暗号化を追加)
    // password引数を使用してAES256モードで暗号化を設定します
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644)
        .with_aes_encryption(AesMode::Aes256, password);

    // 3. アーカイブ内にファイルエントリー作成
    let file_name = Path::new(src)
        .file_name()
        .ok_or("Invalid file name")?
        .to_string_lossy();
    zip.start_file(file_name.to_string(), options)
        .map_err(|e| e.to_string())?;

    // 4. 内容のコピー
    let mut f = File::open(src).map_err(|e| e.to_string())?;
    io::copy(&mut f, &mut zip).map_err(|e| e.to_string())?;

    // 5. 書き込み確定
    zip.finish().map_err(|e| e.to_string())?;

    Ok(())
}

pub fn tar_backup_file(src: &str, backup_dir: &Path) -> Result<(), String> {
    let stem = Path::new(src).file_stem().unwrap().to_string_lossy();
    let tar_filename = timestamped_name(&format!("{}.tar.gz", stem));
    let tar_path = backup_dir.join(tar_filename);

    let file = File::create(&tar_path).map_err(|e| e.to_string())?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut tar = Builder::new(enc);

    let mut f = File::open(src).map_err(|e| e.to_string())?;

    // 修正ポイント: file_name を String に変換することで AsRef<Path> を満たすようにする
    let file_name = Path::new(src)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned(); // ここで String (owned data) に変換

    // tar.append_file は &String なら AsRef<Path> として受け取れるようになります
    tar.append_file(&file_name, &mut f)
        .map_err(|e| e.to_string())?;

    tar.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// Readerの内容をターゲットファイルに書き出す (Goの saveToWorkFile 相当)
/// Rustでは io::Read トレイトを持つものを引数に取ります
pub fn save_to_work_file<R_type: Read>(
    mut reader: R_type,
    target_file: &str,
) -> Result<(), String> {
    // 1. ファイルの作成
    let mut out = File::create(target_file)
        .map_err(|e| format!("Failed to create file {}: {}", target_file, e))?;

    // 2. データのコピー (io.Copy 相当)
    io::copy(&mut reader, &mut out).map_err(|e| format!("Failed to copy data: {}", e))?;

    // 3. ディスクへの書き込み確定 (Sync 相当)
    out.sync_all()
        .map_err(|e| format!("Failed to sync file: {}", e))?;

    Ok(())
}

pub fn restore_archive(archive_path: &str, work_file: &str) -> Result<(), String> {
    let path = Path::new(archive_path);
    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

    if file_name.ends_with(".zip") {
        let file = File::open(archive_path).map_err(|e| e.to_string())?;
        let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;

        if archive.len() > 0 {
            let mut file_in_zip = archive.by_index(0).map_err(|e| e.to_string())?;
            // 既存の utils 関数を呼び出し
            save_to_work_file(&mut file_in_zip, work_file)?;
            return Ok(());
        }
    } else if file_name.ends_with(".tar.gz") {
        let file = File::open(archive_path).map_err(|e| e.to_string())?;
        let tar_gz = GzDecoder::new(file);
        let mut archive = Archive::new(tar_gz);

        if let Some(Ok(mut entry)) = archive.entries().map_err(|e| e.to_string())?.next() {
            // 既存の utils 関数を呼び出し
            save_to_work_file(&mut entry, work_file)?;
            return Ok(());
        }
    }

    Err(format!(
        "サポートされていない形式、またはアーカイブが空です"
    ))
}

pub fn apply_compact_mode(window: &WebviewWindow, is_compact: bool) -> tauri::Result<()> {
    // 1. まず「何でもあり」の状態にする (制約の完全解除)
    #[cfg(desktop)]
    {
        window.set_resizable(true)?;
        window.set_min_size(None::<Size>)?;
        window.set_max_size(None::<Size>)?;
    }
    let (width, height, title) = if is_compact {
        (300.0, 210.0, "cg-file-backup (Compact mode)")
    } else {
        (640.0, 450.0, "cg-file-backup")
    };

    let new_size = Size::Logical(LogicalSize::new(width, height));

    #[cfg(desktop)]
    {
        // 2. タイトルを変更
        window.set_title(title)?;

        // 3. サイズを変更
        // ここで一旦サイズが変わるはずです
        window.set_size(new_size)?;

        // 4. (重要) サイズを固定したい場合は、サイズ変更のあとに設定する
        // デバッグのため、もしこれでも動かないなら下の2行を消してみてください
        window.set_min_size(Some(new_size))?;
        window.set_max_size(Some(new_size))?;
    }
    Ok(())
}

pub fn apply_window_visibility(app: AppHandle, show: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        if show {
            #[cfg(desktop)]
            {
                window.show().map_err(|e| e.to_string())?;
                window.unminimize().map_err(|e| e.to_string())?; // 最小化されていても戻す
                window.set_focus().map_err(|e| e.to_string())?;
            }
        } else {
            #[cfg(desktop)]
            {
                window.hide().map_err(|e| e.to_string())?;
            }
        }
    } else {
        return Err("Main window not found".into());
    }
    Ok(())
}

pub fn apply_window_always_on_top(app: AppHandle, flag: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        #[cfg(desktop)]
        {
            let _ = window.set_always_on_top(flag);
        }
    } else {
        return Err("Main window not found".into());
    }
    Ok(())
}

// 共通化：トレイメニューだけを生成するヘルパー関数
#[cfg(desktop)]
pub fn create_tray_menu<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    config: &AppConfig,
) -> tauri::Result<tauri::menu::Menu<R>> {
    let state = app.state::<AppState>();
    let t = |key: &str| get_language_text(state.clone(), key).unwrap_or_else(|_| key.to_string());

    let mode_full = tauri::menu::CheckMenuItemBuilder::with_id("mode_full", t("modeFull"))
        .checked(config.tray_backup_mode == "copy")
        .build(app)?;
    let mode_arc = tauri::menu::CheckMenuItemBuilder::with_id("mode_arc", t("modeArc"))
        .checked(config.tray_backup_mode == "archive")
        .build(app)?;
    let mode_diff = tauri::menu::CheckMenuItemBuilder::with_id("mode_diff", t("modeDiff"))
        .checked(config.tray_backup_mode == "diff")
        .build(app)?;
    let backup_mode_menu = tauri::menu::SubmenuBuilder::new(app, t("backupMode"))
        .item(&mode_full)
        .item(&mode_arc)
        .item(&mode_diff)
        .build()?;

    tauri::menu::MenuBuilder::new(app)
        .item(&tauri::menu::MenuItemBuilder::with_id("show_window", t("showWindow")).build(app)?)
        .separator()
        .item(&backup_mode_menu)
        .item(&tauri::menu::MenuItemBuilder::with_id("execute", t("executeBtn")).build(app)?)
        .item(&tauri::menu::MenuItemBuilder::with_id("change_work", t("workFileBtn")).build(app)?)
        .item(
            &tauri::menu::MenuItemBuilder::with_id("change_backup", t("backupDirBtn"))
                .build(app)?,
        )
        .separator()
        .item(&tauri::menu::MenuItemBuilder::with_id("quit", t("quit")).build(app)?)
        .build()
}
