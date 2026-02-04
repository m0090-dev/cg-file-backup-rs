#!/bin/bash

# --- デフォルト設定 ---
BRANCH="main"
MESSAGE="chore: update version and push"
SKIP_CI=false

# --- 引数の解析 ---
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --branch) BRANCH="$2"; shift ;;
        --message) MESSAGE="$2"; shift ;;
        --skip-ci) SKIP_CI=true ;;
        *) echo "Unknown parameter passed: $1"; exit 1 ;;
    esac
    shift
done

# skip-ciフラグが立っている場合、メッセージの末尾に付け加える
if [ "$SKIP_CI" = true ]; then
    MESSAGE="$MESSAGE [skip ci]"
fi

# 1/4: バージョン更新スクリプトの実行
echo "[1/4] Running version update script..."
python3 scripts/update_version.py
if [ $? -ne 0 ]; then
    echo "Error: update_version.py failed."
    exit 1
fi

# 2/4: Git add
echo "[2/4] Git add..."
git add .

# 3/4: Git commit
echo "[3/4] Git commit with message: \"$MESSAGE\"..."
git commit -m "$MESSAGE"
if [ $? -ne 0 ]; then
    echo "No changes to commit or git error."
    # コミットがなくてもPushを試みる(念のため)
fi

# 4/4: Git push
echo "[4/4] Pushing to $BRANCH..."
git push origin "$BRANCH"

echo ""
if [ "$SKIP_CI" = true ]; then
    echo "Done! (Actions was skipped as requested)"
else
    echo "Done! Factory is now working on GitHub Actions."
fi
