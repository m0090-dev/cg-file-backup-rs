import {
  CopyBackupFile,
  ArchiveBackupFile,
  BackupOrDiff,
  RestoreBackup,
  GetFileSize,
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
    backupMode: "diff",
    compressMode: "zstd",
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
  UpdateDisplay();
  const mode = tab.backupMode;
  // --- 2. 差分設定の取得 (既存ロジック維持 + 圧縮設定追加) ---
  let algo = document.getElementById("diff-algo").value;
  const compress = tab.compressMode || "zstd";

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
    return successText;
  } catch (err) {
    toggleProgress(false);
    alert(err);
    return null;
  }
}

export function updateExecute() {
  // const tab = getActiveTab();
  // const algo = document.getElementById("diff-algo")?.value;
  // モード取得
  let mode = document.querySelector('input[name="backupMode"]:checked')?.value;
  if (document.body.classList.contains("compact-mode")) {
    mode = document.getElementById("compact-mode-select")?.value;
  }
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
