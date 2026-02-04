use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

/// hdiffz を呼び出して差分を作成する (圧縮設定対応)
pub async fn create_hdiff(
    app: tauri::AppHandle,
    old_file: &str,
    new_file: &str,
    diff_file: &str,
    compress_algo: &str, // "zstd", "lzma2", "none" 等
) -> Result<(), String> {
    // 1. 基本となる引数をベクトルで作成
    // -f: 強制上書き, -s: ストリーミング/高速化
    let mut args = vec!["-f", "-s"];

    // 2. 圧縮アルゴリズムに応じたフラグを追加
    // compress_algo が "none" の場合はフラグを vec に追加しないことで
    // hdiffz のデフォルト動作（uncompress）を呼び出す
    match compress_algo {
        "zstd" => args.push("-c-zstd"),
        "lzma2" => args.push("-c-lzma2"),
        "lzma" => args.push("-c-lzma"),
        "zlib" => args.push("-c-zlib"),
        "ldef" => args.push("-c-ldef"),
        "pbzip2" => args.push("-c-pbzip2"),
        "bzip2" => args.push("-c-bzip2"),
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
