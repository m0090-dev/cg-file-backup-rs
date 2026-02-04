import json
import re
import os

def update_version():
    # 1. tauri.conf.json から最新バージョンを読み取る
    with open('src-tauri/tauri.conf.json', 'r', encoding='utf-8') as f:
        tauri_config = json.load(f)
        version = tauri_config['version']

    config_path = 'src/assets/AppConfig.json' # 実際のパスに合わせてね
    
    if not os.path.exists(config_path):
        print(f"Error: {config_path} not found.")
        return

    # 2. 設定ファイルを読み込む
    with open(config_path, 'r', encoding='utf-8') as f:
        config_data = json.load(f)

    # 3. ja と en の aboutText 内のバージョン表記を置換
    # 例: "WorkBackupTool 1.1.3(2026)" -> "WorkBackupTool 1.1.4(2026)"
    pattern = r"WorkBackupTool [0-9.]+"
    new_text = f"WorkBackupTool {version}"

    for lang in ['ja', 'en']:
        if lang in config_data['i18n']:
            old_about = config_data['i18n'][lang]['aboutText']
            config_data['i18n'][lang]['aboutText'] = re.sub(pattern, new_text, old_about)

    # 4. 上書き保存
    with open(config_path, 'w', encoding='utf-8') as f:
        json.dump(config_data, f, indent=2, ensure_ascii=False)
    
    print(f"Successfully updated config.json to version {version}")

if __name__ == "__main__":
    update_version()
