import json
import re
import os

def update_version():
    # 1. tauri.conf.json から最新バージョンを読み取る
    tauri_config_path = 'src-tauri/tauri.conf.json'
    if not os.path.exists(tauri_config_path):
        print(f"Error: {tauri_config_path} not found.")
        return

    with open(tauri_config_path, 'r', encoding='utf-8') as f:
        tauri_config = json.load(f)
        version = tauri_config.get('version', '0.0.0')  # 存在しない場合はデフォルト

    # 2. Cargo.toml の version を更新（既存の [package] version を置換）
    cargo_toml_path = 'src-tauri/Cargo.toml'
    if not os.path.exists(cargo_toml_path):
        print(f"Warning: {cargo_toml_path} not found. Skipping Cargo.toml update.")
    else:
        with open(cargo_toml_path, 'r', encoding='utf-8') as f:
            cargo_content = f.read()

        # [package] セクションの version = "..." を置換
        # シンプルに正規表現で version 行を更新（コメント行は無視）
        new_cargo_content = re.sub(
            r'(?m)^version\s*=\s*["\']([^"\']+)["\']',
            f'version = "{version}"',
            cargo_content
        )

        # 変更があった場合のみ上書き
        if new_cargo_content != cargo_content:
            with open(cargo_toml_path, 'w', encoding='utf-8') as f:
                f.write(new_cargo_content)
            print(f"Cargo.toml version updated to: {version}")
        else:
            print("Cargo.toml version already matches. No change.")

    # 3. AppConfig.json のパス（既存のまま）
    config_path = 'src/assets/AppConfig.json'
   
    if not os.path.exists(config_path):
        print(f"Error: {config_path} not found.")
        return

    # 4. 設定ファイルを読み込む（既存処理そのまま）
    with open(config_path, 'r', encoding='utf-8') as f:
        config_data = json.load(f)

    # 5. ja と en の aboutText 内のバージョン表記を置換（既存処理そのまま）
    # 例: "WorkBackupTool 1.1.3(2026)" -> "WorkBackupTool 1.1.4(2026)"
    pattern = r"WorkBackupTool [0-9.]+"
    new_text = f"WorkBackupTool {version}"
    for lang in ['ja', 'en']:
        if lang in config_data.get('i18n', {}):
            old_about = config_data['i18n'][lang].get('aboutText', '')
            config_data['i18n'][lang]['aboutText'] = re.sub(pattern, new_text, old_about)

    # 6. 上書き保存（既存処理そのまま）
    with open(config_path, 'w', encoding='utf-8') as f:
        json.dump(config_data, f, indent=2, ensure_ascii=False)
   
    print(f"Successfully updated config.json to version {version}")

if __name__ == "__main__":
    update_version()
