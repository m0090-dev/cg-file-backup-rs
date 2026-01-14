import {
  CopyBackupFile,
  ArchiveBackupFile,
  BackupOrDiff,
  RestoreBackup,
  GetFileSize,
  GetBsdiffMaxFileSize,
  DirExists,
} from "./tauri_exports";

import {
  i18n,
  tabs,
  getActiveTab,
  addToRecentFiles,
  saveCurrentSession,
} from "./state";

import {
  renderTabs,
  UpdateDisplay,
  UpdateHistory,
  toggleProgress,
  showFloatingMessage,
} from "./ui";

let bsdiffLimit = 104857600; // デフォルト100MB (100 * 1024 * 1024)
// --- タブ操作ロジック ---
export function switchTab(id) {
  tabs.forEach((t) => (t.active = t.id === id));
  renderTabs();
  UpdateDisplay();
  UpdateHistory();
  saveCurrentSession();
}

export function addTab() {
  tabs.forEach((t) => (t.active = false));
  tabs.push({
    id: Date.now(),
    workFile: "",
    workFileSize: 0,
    backupDir: "",
    active: true,
  });
  renderTabs();
  UpdateDisplay();
  UpdateHistory();
  saveCurrentSession();
}

export function removeTab(id) {
  const index = tabs.findIndex((t) => t.id === id);
  const wasActive = tabs[index].active;
  tabs.splice(index, 1);
  if (wasActive) tabs[Math.max(0, index - 1)].active = true;
  renderTabs();
  UpdateDisplay();
  UpdateHistory();
  saveCurrentSession();
}

export function reorderTabs(draggedId, targetId) {
  // 文字列IDを比較するために型を合わせる（念のため）
  const draggedIndex = tabs.findIndex(
    (t) => String(t.id) === String(draggedId),
  );
  const targetIndex = tabs.findIndex((t) => String(t.id) === String(targetId));

  if (
    draggedIndex !== -1 &&
    targetIndex !== -1 &&
    draggedIndex !== targetIndex
  ) {
    // 1. 配列のコピーを作成して操作する（リアクティブな問題を避けるため）
    const [removed] = tabs.splice(draggedIndex, 1);
    tabs.splice(targetIndex, 0, removed);

    // 2. セッションを強制保存（ここで localStorage などに書き込まれる）
    saveCurrentSession();

    // 3. 画面全体を再描画
    renderTabs();

    // 4. アクティブなタブの内容も念のため更新
    UpdateDisplay();
  }
}








export async function OnExecute() {
  const tab = getActiveTab();
  if (!tab?.workFile) {
    alert(i18n.selectFileFirst);
    return;
  }

  // --- 1. モードの取得（通常・コンパクト両対応） ---
  const isCompact = document.body.classList.contains("compact-mode");
  let mode = document.querySelector('input[name="backupMode"]:checked')?.value;
  if (isCompact) {
    mode = document.getElementById("compact-mode-select").value;
  }

  // --- 2. 差分設定の取得 (既存ロジック維持 + 圧縮設定追加) ---
  let algo = document.getElementById("diff-algo").value;
  let compress = "zstd"; // デフォルト値

  if (mode === "diff") {
    if (isCompact) {
      // コンパクトモード時は常にhdiffとして扱う、またはグローバルなalgo設定を流用
      // 圧縮設定はコンパクト専用のセレクトボックスから取得
      compress = document.getElementById("compact-hdiff-compress").value;
    } else {
      // 通常モード時は表示されているセレクトボックスから取得
      compress = document.getElementById("hdiff-compress").value;
    }

    // ファイルサイズ制限チェック（bsdiff用ロジック完全維持）
    if (algo === "bsdiff") {
      if (tab.workFileSize > bsdiffLimit) {
        alert(
          `${i18n.fileTooLarge} (Limit: ${Math.floor(bsdiffLimit / 1000000)}MB)`,
        );
        return;
      }
    }
  }

  toggleProgress(true, i18n.processingMsg);

  try {
    let successText = "";

    // --- A. 単純コピーモード ---
    if (mode === "copy") {
      await CopyBackupFile(tab.workFile, tab.backupDir);
      successText = i18n.copyBackupSuccess;
    }
    // --- B. アーカイブモード ---
    else if (mode === "archive") {
      let fmt = document.getElementById("archive-format").value;
      let pwd =
        fmt === "zip-pass"
          ? document.getElementById("archive-password").value
          : "";
      if (fmt === "zip-pass") fmt = "zip";
      await ArchiveBackupFile(tab.workFile, tab.backupDir, fmt, pwd);
      successText = i18n.archiveBackupSuccess.replace(
        "{format}",
        fmt.toUpperCase(),
      );
    }
    // --- C. 差分バックアップモード (既存ロジック完全維持 + 引数拡張) ---
    else if (mode === "diff") {
      console.log("DEBUG JS: tab.selectedTargetDir =", tab.selectedTargetDir);
      console.log("DEBUG JS: tab.backupDir =", tab.backupDir);
      // フォルダの存在確認ロジック
      if (tab.selectedTargetDir) {
        const exists = await DirExists(tab.selectedTargetDir);
        if (!exists) {
          console.log(
            "Selected directory no longer exists. Reverting to auto-discovery.",
          );
          tab.selectedTargetDir = "";
        }
      }

      const targetPath = tab.selectedTargetDir || tab.backupDir || "";
      console.log("DEBUG JS: Final targetPath sent to Rust =", targetPath);

      // Rust側(またはGo側)の関数を呼び出し
      // 引数に新しく compress を追加。algoがbsdiffの場合は内部で無視される設計
      await BackupOrDiff(tab.workFile, targetPath, algo, compress);

      successText = `${i18n.diffBackupSuccess} (${algo.toUpperCase()}${algo === "hdiff" ? ":" + compress : ""})`;
    }

    toggleProgress(false);
    showFloatingMessage(successText);
    UpdateHistory(); // 履歴の更新
  } catch (err) {
    toggleProgress(false);
    alert(err);
  }
}

// --- 初期化: 上限サイズの取得 ---
(async () => {
  const size = await GetBsdiffMaxFileSize();
  if (size > 0) bsdiffLimit = size;
})();
export function updateExecute() {
  const tab = getActiveTab();
  const algo = document.getElementById("diff-algo")?.value;

  // モード取得
  let mode = document.querySelector('input[name="backupMode"]:checked')?.value;
  if (document.body.classList.contains("compact-mode")) {
    mode = document.getElementById("compact-mode-select")?.value;
  }

  // 判定ロジック: tab.workFileSize を使用
  const isTooLargeForBsdiff =
    mode === "diff" &&
    algo === "bsdiff" &&
    (tab?.workFileSize || 0) > bsdiffLimit;

  // 2つのボタン両方を制御
  const btns = ["execute-backup-btn", "compact-execute-btn"];
  btns.forEach((id) => {
    const btn = document.getElementById(id);
    if (!btn) return;

    btn.disabled = isTooLargeForBsdiff;
    btn.style.opacity = isTooLargeForBsdiff ? "0.5" : "1";
    btn.style.cursor = isTooLargeForBsdiff ? "not-allowed" : "pointer";
    btn.title = isTooLargeForBsdiff
      ? `File too large for bsdiff (Max: ${Math.floor(bsdiffLimit / 1000000)}MB)`
      : "";
  });
}

// --- 復元・適用ロジック ---
export async function applySelectedBackups() {
  const tab = getActiveTab();
  const targets = Array.from(
    document.querySelectorAll(".diff-checkbox:checked"),
  ).map((el) => el.value);
  if (targets.length > 0 && confirm(i18n.restoreConfirm)) {
    toggleProgress(true, "Restoring...");
    try {
      for (const p of targets) {
        await RestoreBackup(p, tab.workFile);
      }
      toggleProgress(false);
      showFloatingMessage(i18n.diffApplySuccess);
      UpdateHistory();
    } catch (err) {
      toggleProgress(false);
      alert(err);
    }
  }
}
