import {
  SelectAnyFile,
  SelectBackupFolder,
  GetFileSize,
  WriteTextFile,
  ReadTextFile,
  RestoreBackup,
  EventsOn,
} from "./tauri_exports";

import {
  i18n,
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

import { addTab, OnExecute} from "./actions";
import { ask } from "@tauri-apps/plugin-dialog";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

// --- ドラッグアンドドロップの基本防止設定 ---
const preventDefault = (e) => {
  e.preventDefault();
  e.stopPropagation();
};

export function setupGlobalEvents() {
  // Only this
  window.addEventListener("dragenter", preventDefault, true);

  // --- ヘルパー関数: 共通ロジックの定義 ---

  // 作業ファイル選択ロジック
  const handleSelectWorkFile = async () => {
    const tab = getActiveTab();
    const res = await SelectAnyFile(i18n.workFileBtn, [
      { DisplayName: "Work file", Pattern: "*.*" },
    ]);
    if (res) {
      tab.workFile = res;
      tab.workFileSize = await GetFileSize(res);
      tab.backupDir = "";
      tab.selectedTargetDir = "";
      addToRecentFiles(res);
      renderTabs();
      UpdateDisplay();
      UpdateHistory();
      saveCurrentSession();
      showFloatingMessage(i18n.updatedWorkFile);
    }
  };

  // バックアップ先フォルダ選択ロジック
  const handleSelectBackupDir = async () => {
    const tab = getActiveTab();
    const res = await SelectBackupFolder();
    if (res) {
      tab.backupDir = res;
      UpdateDisplay();
      UpdateHistory();
      saveCurrentSession();
      showFloatingMessage(i18n.updatedBackupDir);
    }
  };

  // --- クリックイベントリスナー ---
  window.addEventListener("click", async (e) => {
    const target = e.target.closest("button") || e.target;
    const id = target.id;
    const tab = getActiveTab();

    if (id === "add-tab-btn") {
      addTab();
      return;
    }

    const noteBtn = e.target.closest(".note-btn");
    if (noteBtn) {
      const path = noteBtn.getAttribute("data-path");
      const cur = await ReadTextFile(path + ".note").catch(() => "");
      const val = prompt("Memo:", cur);
      if (val !== null) {
        await WriteTextFile(path + ".note", val);
        UpdateHistory();
      }
      return;
    }

    if (id === "workfile-btn" || id === "compact-workfile-btn") {
      await handleSelectWorkFile();
      return;
    } else if (id === "backupdir-btn") {
      await handleSelectBackupDir();
      return;
    } else if (id === "execute-backup-btn" || id === "compact-execute-btn") {
      OnExecute();
      return;
    } else if (id === "refresh-diff-btn") {
      UpdateHistory();
      return;
    } else if (id === "select-all-btn") {
      const cbs = document.querySelectorAll(".diff-checkbox");
      const all = Array.from(cbs).every((cb) => cb.checked);
      cbs.forEach((cb) => (cb.checked = !all));
      return;
    } else if (id === "apply-selected-btn") {
      e.preventDefault();
      e.stopPropagation();
      const targets = Array.from(
        document.querySelectorAll(".diff-checkbox:checked"),
      ).map((el) => el.value);
      if (targets.length === 0) return;

      const isConfirmed = await ask(i18n.restoreConfirm, {
        title: "CG File Backup",
        type: "warning",
      });

      if (isConfirmed) {
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
      return;
    }
  });

  // --- 変更イベントリスナー ---
  document.addEventListener("change", (e) => {
    const id = e.target.id;
    const name = e.target.name;
    const value = e.target.value;
    const tab = getActiveTab();
    if (name == "diff-algo"){
      if(tab) tab.diffAlgo = value;
    }
    if (name === "backupMode") {
      if (tab) tab.backupMode = value; // データ側を確実に更新
    }
    if (id === "hdiff-compress" || id === "compact-hdiff-compress") {
      if (tab) tab.compressMode = value;
    }
    if (
      ["backupMode", "archive-format"].includes(name) ||
      id === "archive-format" ||
      id === "diff-algo" ||
      id === "hdiff-compress" ||
      id === "compact-hdiff-compress"
    ) {
      UpdateDisplay();
      saveCurrentSession();
    }

    if (id === "compact-mode-select") {
      const radio = document.querySelector(
        `input[name="backupMode"][value="${value}"]`,
      );
      if (radio) {
        radio.checked = true;
        const tab = getActiveTab();
        if (tab) tab.backupMode = value;
      }
    }
  });

  // --- Rust / Tray からのイベントリスナー ---

  EventsOn("tray-execute-clicked", async () => {
    const resultMsg = await OnExecute();
    if (!resultMsg) return;
    let permissionGranted = await isPermissionGranted();

    // アクセス権限が設定されていない場合はアクセス権限を要求する必要があります
    if (!permissionGranted) {
      const permission = await requestPermission();
      permissionGranted = permission === "granted";
    }
    // アクセス権限が付与され次第、通知が送信されます
    if (permissionGranted) {
      sendNotification({
        title: "cg-file-backup",
        body: resultMsg,
      });
    }
  });

  EventsOn("tray-change-work-clicked", () => {
    handleSelectWorkFile();
  });

  EventsOn("tray-change-backup-clicked", () => {
    handleSelectBackupDir();
  });

  // 【物理同期版】トレイ：バックアップモードの同期
  EventsOn("tray-mode-change", (newMode) => {
    const radio = document.querySelector(
      `input[name="backupMode"][value="${newMode}"]`,
    );

    if (radio) {
      // ラジオボタンを物理的にチェック
      radio.checked = true;

      // changeイベントを強制発火させる
      radio.dispatchEvent(new Event("change", { bubbles: true }));

      // 念のため確実に内部データも更新
      const tab = getActiveTab();
      if (tab) tab.backupMode = newMode;

      // 4. UI更新と通知
      UpdateDisplay();
      saveCurrentSession();
      showFloatingMessage(`${i18n.updatedBackupMode || "Mode"}: ${newMode}`);
    }
  });

  EventsOn("compact-mode-event", (isCompact) => {
    const view = document.getElementById("compact-view");
    if (isCompact) {
      document.body.classList.add("compact-mode");
      if (view) view.classList.remove("hidden");
      if (typeof UpdateDisplay === "function") UpdateDisplay();
    } else {
      document.body.classList.remove("compact-mode");
      if (view) view.classList.add("hidden");
    }
  });
}
