mod app;
use crate::app::commands::*;
use crate::app::config::*;
use crate::app::state::AppState;
use crate::app::types::AppConfig;
use crate::app::utils;
use crate::utils::create_tray_menu;
use app::menu::*;
use app::tray::*;
use std::fs;
use std::sync::Mutex;
use tauri::AppHandle;
use tauri::{menu::MenuEvent, Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_notification::NotificationExt;

pub fn handle_menu_event(app: &tauri::AppHandle, event: tauri::menu::MenuEvent) {
    let state = app.state::<AppState>();
    let id = event.id.as_ref();
    //println!("--- Menu Event: '{}' ---", id);

    match id {
        // --- 1. バックアップモード切替 (フル / アーカイブ / 差分) ---
        "mode_full" | "mode_arc" | "mode_diff" => {
            let mode_str = match id {
                "mode_full" => "copy",
                "mode_arc" => "archive",
                _ => "diff",
            };

            // Configを更新して保存
            let config = {
                let mut cfg = state.config.lock().unwrap();
                cfg.tray_backup_mode = mode_str.to_string();
                cfg.clone()
            };
            let _ = state.save();

            // JS側に同期を依頼
            let _ = app.emit("tray-mode-change", mode_str);

            // 【重要】トレイを再生成せず、トレイの「メニュー」だけを更新する
            if let Some(tray) = app.tray_by_id("main-tray") {
                // setup_menu ではなく、トレイ用のメニュー生成ロジックを呼ぶ
                // ※setup_tray 内部で作っている Menu を取得する関数があればベスト
                // ここでは再度メニューオブジェクトを構築してセットします
                if let Ok(new_menu) = create_tray_menu(app, &config) {
                    let _ = tray.set_menu(Some(new_menu));
                }
            }
        }

        // --- 2. トレイモード切替 ---
        "tray_mode" => {
            let next_is_tray_enabled = {
                let mut cfg = state.config.lock().unwrap();
                cfg.tray_mode = !cfg.tray_mode;
                cfg.tray_mode
            };

            let _ = utils::apply_window_visibility(app.clone(), !next_is_tray_enabled);

            // メインメニューのチェック状態を同期
            if let Some(item) = app
                .menu()
                .and_then(|m| m.get(id))
                .and_then(|i| i.as_check_menuitem().cloned())
            {
                let _ = item.set_checked(next_is_tray_enabled);
            }
            let _ = state.save();
        }

        // --- 3. ウィンドウ表示 (トレイから復帰時) ---
        "show_window" => {
            let config = {
                let mut cfg = state.config.lock().unwrap();
                cfg.tray_mode = false;
                cfg.clone()
            };
            let _ = state.save();

            let _ = utils::apply_window_visibility(app.clone(), true);
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }

            // メインメニュー全体をリフレッシュ
            if let Ok(new_menu) = setup_menu(app, &config) {
                let _ = app.set_menu(new_menu);
            }
        }

        // --- 4. 最前面表示 / コンパクトモード / 状態復元 ---
        "always_on_top" | "compact_mode" | "restore_state" => {
            // スコープ（波括弧）を使って、ロックの寿命を短くします
            let (next_val, id_clone) = {
                let mut cfg = state.config.lock().unwrap();
                let val = match id {
                    "always_on_top" => {
                        cfg.always_on_top = !cfg.always_on_top;
                        cfg.always_on_top
                    }
                    "compact_mode" => {
                        cfg.compact_mode = !cfg.compact_mode;
                        cfg.compact_mode
                    }
                    "restore_state" => {
                        cfg.restore_previous_state = !cfg.restore_previous_state;
                        cfg.restore_previous_state
                    }
                    _ => false,
                };
                (val, id.to_string())
            }; 
            let _ = state.save();

            // 以降の処理
            if id_clone == "always_on_top" {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_always_on_top(next_val);
                }
            } else if id_clone == "compact_mode" {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = utils::apply_compact_mode(&window, next_val);
                    let _ = app.emit("compact-mode-event", next_val);
                }
            }

            if let Some(item) = app
                .menu()
                .and_then(|m| m.get(&id_clone))
                .and_then(|i| i.as_check_menuitem().cloned())
            {
                let _ = item.set_checked(next_val);
            }
        }

        // --- 5. アクション系 ---
        "execute" => {
            let _ = app.emit("tray-execute-clicked", ());
        }
        "change_work" => {
            let _ = app.emit("tray-change-work-clicked", ());
        }
        "change_backup" => {
            let _ = app.emit("tray-change-backup-clicked", ());
        }

        "quit" => {
            app.exit(0);
        }

        "lang_en" | "lang_ja" => {
            let lang_code = if id == "lang_en" { "en" } else { "ja" };

            // 1. まずConfigを更新・保存（重要：setup_menuがこの値を見るため）
            let config = {
                let mut cfg = state.config.lock().unwrap();
                cfg.language = lang_code.to_string();
                cfg.clone()
            };
            let _ = state.save();

            // 2. メニュー全体を再生成してセットし直す（これで確実に見た目が直る）
            if let Ok(new_menu) = setup_menu(app, &config) {
                let _ = app.set_menu(new_menu);
            }

            // 3. 通知
            let t = |key: &str| -> String {
                get_language_text(state.clone(), key).unwrap_or_else(|_| key.to_string())
            };
            let _ = app.dialog().message(&t("restartRequired")).show(|_| {});
        }
        // --- 6. About ダイアログ ---
        "about" => {
            let t = |key: &str| -> String {
                get_language_text(state.clone(), key).unwrap_or_else(|_| key.to_string())
            };

            // 指定通り、title は about、message は aboutText の i18n テキストのみを表示
            app.dialog()
                .message(t("aboutText"))
                .title(t("about"))
                .show(|_| {});
        }

        _ => {}
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            let initial_size = tauri::Size::Logical(tauri::LogicalSize::new(640.0, 450.0));

            let _ = window.set_min_size(Some(initial_size));
            let _ = window.set_max_size(Some(initial_size));

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let config_dir = app.path().app_config_dir()?;
            let config_path = config_dir.join("AppConfig.json");

            let config = if config_path.exists() {
                let content = fs::read_to_string(&config_path)?;
                serde_json::from_str(&content).unwrap_or_else(|_| default_config())
            } else {
                default_config()
            };

            app.manage(AppState {
                config: Mutex::new(config.clone()),
                config_path,
            });

            let menu = setup_menu(app.handle(), &config)?;
            let _tray = setup_tray(app.handle(), &config);
            app.set_menu(menu.clone())?;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }

            // --- 起動時の完全同期ロジック ---
            let tray_enabled = config.tray_mode;
            let always_on_top_enabled = config.always_on_top;


            // ウィンドウの可視性設定を復元
            let _ = utils::apply_window_visibility(app.handle().clone(), !tray_enabled);

            // ウィンドウの最前面設定を復元
              let _ = utils::apply_window_always_on_top(app.handle().clone(),always_on_top_enabled);

            // 2. メニューアイテムのチェック状態を同期
            if let Some(item) = menu
                .get("tray_mode")
                .and_then(|i| i.as_check_menuitem().cloned())
            {
                let _ = item.set_checked(tray_enabled);
            }

            // 3. トレイモードでない場合は確実に表示（tauri.confのvisible:falseを考慮）
            if !tray_enabled {
                let _ = window.show();
            }

            app.on_menu_event(move |app_handle, event| {
                handle_menu_event(app_handle, event);
            });

            #[cfg(desktop)]
            {
                // 1. ショートカットの定義
                let quit_shortcut = Shortcut::new(Some(Modifiers::CONTROL), Code::KeyQ);
                let about_shortcut = Shortcut::new(Some(Modifiers::CONTROL), Code::KeyA);

                // 2. プラグインをハンドラ付きで登録
                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_handler(move |app_handle, shortcut, event| {
                            if event.state() == ShortcutState::Pressed {
                                if shortcut == &quit_shortcut {
                                    app_handle.exit(0);
                                } else if shortcut == &about_shortcut {
                                    // app_handle から現在の State を取得
                                    let state = app_handle.state::<AppState>();

                                    // get_language_text に State をそのまま渡すためのクロージャ
                                    let t = |key: &str| -> String {
                                        // inner() を呼ばず、state.clone() で State 型のまま渡す
                                        get_language_text(state.clone(), key)
                                            .unwrap_or_else(|_| key.to_string())
                                    };

                                    // ダイアログ表示
                                    let _ = app_handle
                                        .dialog()
                                        .message(t("aboutText"))
                                        .title(t("about"))
                                        .show(|_| {});
                                }
                            }
                        })
                        .build(),
                )?;

                // 3. ショートカットを OS に登録
                app.global_shortcut().register(quit_shortcut)?;
                app.global_shortcut().register(about_shortcut)?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            set_always_on_top,
            get_restore_previous_state,
            get_bsdiff_max_file_size,
            get_auto_base_generation_threshold,
            get_language_text,
            get_i18n,
            set_language,
            get_config_dir,
            backup_or_diff,
            apply_multi_diff,
            copy_backup_file,
            archive_backup_file,
            dir_exists,
            restore_backup,
            get_file_size,
            select_any_file,
            select_backup_folder,
            open_directory,
            toggle_compact_mode,
            write_text_file,
            read_text_file,
            get_backup_list,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
