use crate::app::commands::get_language_text; // 既存のコマンドをインポート
use crate::app::state::AppState;
use crate::app::types::AppConfig;
use tauri::State;
use tauri::{
    menu::{CheckMenuItemBuilder, Menu, MenuBuilder, MenuItem, MenuItemBuilder, SubmenuBuilder},
    AppHandle, Manager, Runtime,
};

pub fn setup_menu<R: Runtime>(app: &AppHandle<R>, config: &AppConfig) -> tauri::Result<Menu<R>> {
    // get_language_text コマンドが必要とする State を取得します
    let state: State<'_, AppState> = app.state();

    // 既存のコマンド get_language_text を直接呼び出すヘルパー
    // エラー時はキー名をそのまま返す Go 版 GetLanguageText と同じ挙動にします
    let t = |key: &str| -> String {
        get_language_text(state.clone(), key).unwrap_or_else(|_| key.to_string())
    };

    // --- メニューアイテムの構築 ---

    // チェック状態は config から取得
    let always_on_top = CheckMenuItemBuilder::with_id("always_on_top", t("alwaysOnTop"))
        .checked(config.always_on_top)
        .build(app)?;

    let restore_state = CheckMenuItemBuilder::with_id("restore_state", t("restoreState"))
        .checked(config.restore_previous_state)
        .build(app)?;

    let compact_mode = CheckMenuItemBuilder::with_id("compact_mode", t("compactMode"))
        .checked(config.compact_mode) // Go版と同様に初期値は false
        .build(app)?;

    let tray_mode = CheckMenuItemBuilder::with_id("tray_mode", t("trayMode"))
        .checked(false) // Go版と同様に初期値は false
        .build(app)?;

    let lang_en = CheckMenuItemBuilder::with_id("lang_en", t("english"))
        .checked(config.language == "en")
        .build(app)?;
    let lang_ja = CheckMenuItemBuilder::with_id("lang_ja", t("japanese"))
        .checked(config.language == "ja")
        .build(app)?;

    // quit の作成
    let quit = MenuItem::with_id(
        app,
        "quit",         // id
        t("quit"),      // text
        true,           // enabled
        Some("Ctrl+Q"), // accelerator (Option型なので Some で囲む)
    )?;

    // about の作成
    let about = MenuItem::with_id(
        app,
        "about",        // id
        t("about"),     // text
        true,           // enabled
        Some("Ctrl+A"), // accelerator
    )?;

    // --- サブメニューの組み立て ---

    let settings_menu = SubmenuBuilder::new(app, t("settings"))
        .item(&always_on_top)
        .item(&restore_state)
        .item(&compact_mode)
        .item(&tray_mode)
        .item(&lang_en)
        .item(&lang_ja)
        .item(&quit)
        .build()?;

    let about_menu = SubmenuBuilder::new(app, t("about")).item(&about).build()?;

    // --- ルートメニューの生成 ---

    MenuBuilder::new(app)
        .items(&[&settings_menu, &about_menu])
        .build()
}
