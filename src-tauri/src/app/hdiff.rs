use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tauri::Manager;

async fn ensure_sidecar_executable(app: &tauri::AppHandle, sidecar_name: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        use tauri::Manager;

        // Tauri v2 が Sidecar を探す標準的なパスを組み立てる
        // 開発環境でも AppImage 内でも、Sidecar は Resource ディレクトリ配下に置かれます
        let resource_path = app
            .path()
            .resolve(
                sidecar_name, // ターゲットトリプルなしの名前
                tauri::path::BaseDirectory::Resource,
            )
            .map_err(|e| e.to_string())?;

        println!("Checking sidecar path: {:?}", resource_path);

        if let Ok(metadata) = std::fs::metadata(&resource_path) {
            let mut perms = metadata.permissions();
            if perms.mode() & 0o111 == 0 {
                perms.set_mode(0o755);
                std::fs::set_permissions(&resource_path, perms)
                    .map_err(|e: std::io::Error| e.to_string())?;
                println!("Successfully fixed permissions!");
            }
        } else {
            // もし見つからなければ、ターゲットトリプル付きのフルネームでも試す
            let triple_name = format!("{}-x86_64-unknown-linux-gnu", sidecar_name);
            let resource_path_with_triple = app
                .path()
                .resolve(triple_name, tauri::path::BaseDirectory::Resource)
                .map_err(|e| e.to_string())?;
            
            if let Ok(metadata) = std::fs::metadata(&resource_path_with_triple) {
                let mut perms = metadata.permissions();
                if perms.mode() & 0o111 == 0 {
                    perms.set_mode(0o755);
                    std::fs::set_permissions(&resource_path_with_triple, perms).map_err(|e| e.to_string())?;
                }
            }
        }
    }
    Ok(())
}


/// hdiffz を呼び出して差分を作成する (圧縮設定対応)
pub async fn create_hdiff(
    app: tauri::AppHandle,
    old_file: &str,
    new_file: &str,
    diff_file: &str,
    compress_algo: &str, // "zstd", "lzma2", "none" 等
) -> Result<(), String> {
    ensure_sidecar_executable(&app, "hdiffz").await?;
    // 1. 基本となる引数をベクトルで作成
    // -f: 強制上書き, -s: ストリーミング/高速化
    let mut args = vec!["-f", "-s"];

    // 2. 圧縮アルゴリズムに応じたフラグを追加
    // compress_algo が "none" の場合はフラグを vec に追加しないことで
    // hdiffz のデフォルト動作（uncompress）を呼び出す
    match compress_algo {
        "zstd" => args.push("-c-zstd"),
        "lzma2" => args.push("-c-lzma2"),
        "zlib" => args.push("-c-zlib"),
        "none" => {
            // 何も追加しない（uncompress）
        }
        _ => {
            // 未知の指定があればデフォルトとして zstd を追加
            args.push("-c-zstd");
        }
    };

    // 3. 最後にパス情報を追加
    args.push(old_file);
    args.push(new_file);
    args.push(diff_file);

    // 4. Sidecar "hdiffz" を呼び出し
    let sidecar_command = app
        .shell()
        .sidecar("hdiffz")
        .map_err(|e| e.to_string())?
        .args(&args); // 動的に構築した args リストを渡す

    // Windowsでのウィンドウ非表示等は Tauri が内部で処理
    let output = sidecar_command.output().await.map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        Err(format!("hdiffz error: {}", err_msg))
    }
}

/// hpatchz を呼び出してパッチを適用（復元）する
pub async fn apply_hdiff(
    app: tauri::AppHandle,
    base_full: &str,
    diff_file: &str,
    out_path: &str,
) -> Result<(), String> {
    ensure_sidecar_executable(&app, "hpatchz").await?;
    // Sidecar "hpatchz" を呼び出し
    let sidecar_command = app
        .shell()
        .sidecar("hpatchz")
        .map_err(|e| e.to_string())?
        .args(["-f", "-s", base_full, diff_file, out_path]);

    let output = sidecar_command.output().await.map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        Err(format!("hpatchz error: {}", err_msg))
    }
}
