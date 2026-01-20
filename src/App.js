import { GetI18N, GetFileSize, OnFileDrop } from "./tauri_exports";

import {
  i18n,
  setI18N,
  tabs,
  getActiveTab,
  addToRecentFiles,
  restoreSession,
  saveCurrentSession,
} from "./state";

import {
  renderRecentFiles,
  renderTabs,
  UpdateDisplay,
  UpdateHistory,
  showFloatingMessage,
  showFloatingError,
} from "./ui";

import { setupGlobalEvents } from "./events";

// --- 初期化ロジック ---
async function Initialize() {
  const data = await GetI18N();

  if (!data) return;

  // stateにi18nデータをセット

  setI18N(data);

  await restoreSession();

  const setText = (id, text) => {
    const el = document.getElementById(id);
    if (el) el.textContent = text || "";
  };
  const setPlaceholder = (id, text) => {
    const el = document.getElementById(id);
    if (el) el.placeholder = text || "";
  };

  const setQueryText = (sel, text) => {
    const el = document.querySelector(sel);
    if (el) el.textContent = text || "";
  };

  setQueryText(".action-section h3", i18n.newBackupTitle);

  setQueryText(".history-section h3", i18n.historyTitle);
  setQueryText(".recent-title", i18n.recentFilesTitle);
  setText("workfile-btn", i18n.workFileBtn);

  setText("backupdir-btn", i18n.backupDirBtn);

  setText("label-target", i18n.labelWorkFile); 
  setText("label-location", i18n.labelLocation);
  setPlaceholder("history-search", i18n.searchPlaceholder || "Search...");
  setText("progress-status", i18n.readyStatus || "Ready");

  const titles = document.querySelectorAll(".mode-title");

  const descs = document.querySelectorAll(".mode-desc");

  if (titles.length >= 3) {
    titles[0].textContent = i18n.fullCopyTitle;
    descs[0].textContent = i18n.fullCopyDesc;

    titles[1].textContent = i18n.archiveTitle;
    descs[1].textContent = i18n.archiveDesc;

    titles[2].textContent = i18n.diffTitle;
    descs[2].textContent = i18n.diffDesc;
  }

  setText("execute-backup-btn", i18n.executeBtn);

  setText("refresh-diff-btn", i18n.refreshBtn);

  setText("apply-selected-btn", i18n.applyBtn);

  setText("select-all-btn", i18n.selectAllBtn);

  setText("drop-modal-title", i18n.dropModalTitle);

  setText("drop-set-workfile", i18n.dropSetWorkFile);

  setText("drop-set-backupdir", i18n.dropSetBackupDir);

  setText("drop-cancel", i18n.dropCancel);

  // Compact用テキスト

  setQueryText(".compact-title-text", i18n.compactMode || "Compact");

  setText("compact-workfile-btn", i18n.workFileBtn);

  setText("compact-execute-btn", i18n.executeBtn);

  const cSel = document.getElementById("compact-mode-select");

  if (cSel && cSel.options.length >= 3) {
    cSel.options[0].text = i18n.fullCopyTitle;

    cSel.options[1].text = i18n.archiveTitle;

    cSel.options[2].text = i18n.diffTitle;
  }

  const workBtn = document.getElementById("workfile-btn");

  const recentSec = document.querySelector(".recent-files-section");

  if (workBtn && recentSec) {
    workBtn.addEventListener("mouseenter", () => {
      recentSec.style.display = "block";
      setTimeout(() => (recentSec.style.opacity = "1"), 10);
    });

    workBtn.addEventListener("mouseleave", () => {
      setTimeout(() => {
        if (!recentSec.matches(":hover")) {
          recentSec.style.display = "none";
          recentSec.style.opacity = "0";
        }
      }, 300);
    });

    recentSec.addEventListener("mouseleave", () => {
      recentSec.style.display = "none";
      recentSec.style.opacity = "0";
    });
  }

  setupDragAndDrop();

  setupGlobalEvents(); // events.js からイベントリスナーを登録

  renderTabs();

  renderRecentFiles();

  UpdateDisplay();

  UpdateHistory();
}

// --- ドラッグアンドドロップ設定 ---

function setupDragAndDrop() {
  OnFileDrop((x, y, paths) => {
    if (!paths || paths.length === 0) return;

    const droppedPath = paths[0];

    const modal = document.getElementById("drop-modal");

    const pathText = document.getElementById("drop-modal-path");

    (async () => {
      let isDir = false;

      try {
        const size = await GetFileSize(droppedPath);

        if (size === undefined || size < 0) isDir = true;
      } catch (e) {
        isDir = true;
      }

      pathText.textContent = droppedPath;

      modal.classList.remove("hidden");

      document.getElementById("drop-set-workfile").onclick = async () => {
        if (isDir) {
          showFloatingError(
            i18n.dropErrorFolderAsFile ||
              "フォルダはファイルとして設定できません",
          );

          return;
        }

        const tab = getActiveTab();

        tab.workFile = droppedPath;

        tab.workFileSize = await GetFileSize(droppedPath);

        tab.backupDir = "";
        tab.selectedTargetDir = "";

        addToRecentFiles(droppedPath);

        finishDrop(i18n.updatedWorkFile);
      };

      document.getElementById("drop-set-backupdir").onclick = () => {
        if (!isDir) {
          showFloatingError(
            i18n.dropErrorFileAsFolder ||
              "ファイルはフォルダとして設定できません",
          );

          return;
        }

        const tab = getActiveTab();

        tab.backupDir = droppedPath;

        finishDrop(i18n.updatedBackupDir);
      };

      document.getElementById("drop-cancel").onclick = () => {
        modal.classList.add("hidden");
      };

      function finishDrop(msg) {
        modal.classList.add("hidden");

        showFloatingMessage(msg);

        renderTabs();

        UpdateDisplay();

        UpdateHistory();

        saveCurrentSession();
      }
    })();
  }, true);
}

// アプリケーション開始

document.addEventListener("DOMContentLoaded", Initialize);
