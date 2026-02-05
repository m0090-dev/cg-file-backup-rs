use crate::app::commands::get_language_text;
use crate::app::state::AppState;
use crate::app::types::AppConfig;
use crate::app::utils;
use tauri::{AppHandle, Emitter, Manager, Runtime, State, Wry};

#[cfg(desktop)]
use tauri_plugin_positioner::{Position, WindowExt};

#[cfg(desktop)]
use tauri::menu::{
    CheckMenuItemBuilder, Menu, MenuBuilder, MenuItem, MenuItemBuilder, PredefinedMenuItem,
    SubmenuBuilder,
};

#[cfg(desktop)]
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

#[cfg(desktop)]
pub fn setup_tray(app: &AppHandle, config: &AppConfig) -> tauri::Result<TrayIcon<Wry>> {
    // 1. Stateから多言語テキスト取得用ヘルパーを用意
    let state: State<'_, AppState> = app.state();
    let t = |key: &str| -> String {
        get_language_text(state.clone(), key).unwrap_or_else(|_| key.to_string())
    };

    // --- 追加: バックアップモード・サブメニューの構築 ---
    // config.tray_backup_mode に基づいてチェック状態を決定
    let mode_full = CheckMenuItemBuilder::with_id("mode_full", t("modeFull"))
        .checked(config.tray_backup_mode == "full")
        .build(app)?;

    let mode_arc = CheckMenuItemBuilder::with_id("mode_arc", t("modeArc"))
        .checked(config.tray_backup_mode == "arc")
        .build(app)?;

    let mode_diff = CheckMenuItemBuilder::with_id("mode_diff", t("modeDiff"))
        .checked(config.tray_backup_mode == "diff")
        .build(app)?;

    let backup_mode_menu = SubmenuBuilder::new(app, t("backupMode"))
        .item(&mode_full)
        .item(&mode_arc)
        .item(&mode_diff)
        .build()?;

    // 2. メニューアイテムの構築
    let show_window = MenuItemBuilder::with_id("show_window", t("showWindow")).build(app)?;

    // アクション系
    let execute = MenuItemBuilder::with_id("execute", t("executeBtn")).build(app)?;
    let change_work = MenuItemBuilder::with_id("change_work", t("workFileBtn")).build(app)?;
    let change_backup = MenuItemBuilder::with_id("change_backup", t("backupDirBtn")).build(app)?;

    // 区切り線
    let separator = PredefinedMenuItem::separator(app)?;

    // 終了
    let quit = MenuItemBuilder::with_id("quit", t("quit")).build(app)?;

    // 3. トレイメニューの構築
    // 順序: ウィンドウ表示 -> (線) -> モード選択 -> 実行 -> ファイル選択 -> 保存先選択 -> (線) -> 終了
    let tray_menu = MenuBuilder::new(app)
        .items(&[
            &show_window,
            &separator,
            &backup_mode_menu, // ← ここにモード選択を追加
            &execute,
            &change_work,
            &change_backup,
            &separator,
            &quit,
        ])
        .build()?;
    // 4. トレイアイコンの構築
    TrayIconBuilder::with_id("main-tray")
        .tooltip(t("trayTitle"))
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&tray_menu)
        .menu_on_left_click(false) // 左クリックでメニューを出さない設定
        .on_tray_icon_event(|tray, event| {
            // Positionerプラグインにイベントを渡す（座標計算に必要）
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);

            match event {
                // 左クリックが離された瞬間
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    let app_handle = tray.app_handle();
                    if let Some(window) = app_handle.get_webview_window("main") {
                        // 1. 追加した関数で「枠なし・コンパクトサイズ」に変更
                        let _ = utils::apply_tray_popup_mode(&window, true);
                        // 2. JS側に通知してコンパクト用レイアウトに切り替え
                        let _ = app_handle.emit("compact-mode-event", true);

                        // 3. トレイアイコンの真上（TrayCenter）にワープ
                        // ※ .as_ref().window() は Tauri v2 の型合わせ用
                        let _ = window.as_ref().window().move_window(Position::TrayCenter);

                        // 4. 表示してフォーカスを当てる
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                _ => {}
            }
        })
        .build(app)
}
