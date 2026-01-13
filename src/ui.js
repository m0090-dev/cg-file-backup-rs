import {
  GetBackupList,
  GetFileSize,
  WriteTextFile,
  ReadTextFile,
  GetConfigDir,
} from "./tauri_exports";

import {
  i18n,
  tabs,
  recentFiles,
  getActiveTab,
  formatSize,
  saveCurrentSession,
  addToRecentFiles,
} from "./state";

import { showMemoDialog } from "./memo.js";

import { switchTab, removeTab, updateExecute, reorderTabs } from "./actions";

// UI描画・メッセージ系（通常版）
export function showFloatingMessage(text) {
  const msgArea = document.getElementById("message-area");
  if (!msgArea) return;

  // --- 追加：前回の「赤」が残っていたら消す ---
  msgArea.classList.remove("error");

  msgArea.textContent = text;
  msgArea.classList.remove("hidden");

  // 既存のタイマーと競合しないよう、単純に3秒後に隠す
  setTimeout(() => msgArea.classList.add("hidden"), 3000);
}

// エラー版
export function showFloatingError(text) {
  const msgArea = document.getElementById("message-area");
  if (!msgArea) return;

  // 一旦リセットしてから赤を付ける
  msgArea.classList.add("error");
  msgArea.textContent = text;
  msgArea.classList.remove("hidden");

  setTimeout(() => {
    msgArea.classList.add("hidden");
    // 完全に隠れてから色を戻す
    setTimeout(() => {
      // まだ hidden 状態のときだけクラスを消す（連打対策）
      if (msgArea.classList.contains("hidden")) {
        msgArea.classList.remove("error");
      }
    }, 500);
  }, 3000);
}

export function renderRecentFiles() {
  const list = document.getElementById("recent-list");
  const section = document.getElementById("recent-files-section");
  let titleEl = document.querySelector(".recent-title");
  if (!titleEl) {
    titleEl = document.createElement("div");
    titleEl.className = "recent-title";
    section.insertBefore(titleEl, list);
  }
  titleEl.textContent = i18n?.recentFilesTitle || "RECENT FILES";
  if (!list) return;
  if (recentFiles.length === 0) {
    list.innerHTML = `<span class="recent-empty">No recent files</span>`;
    return;
  }
  list.innerHTML = recentFiles
    .map((path) => {
      const fileName = path.split(/[\\/]/).pop();
      return `<div class="recent-item" title="${path}" data-path="${path}"><i></i> ${fileName}</div>`;
    })
    .join("");

  list.querySelectorAll(".recent-item").forEach((el) => {
    el.onclick = async (e) => {
      e.stopPropagation();
      const path = el.getAttribute("data-path");
      const tab = getActiveTab();
      try {
        tab.workFileSize = await GetFileSize(path);
        tab.workFile = path;
        tab.backupDir = "";
        tab.selectedTargetDir = "";
        addToRecentFiles(path);
        renderRecentFiles(); // state側で呼べないためここで実行
        renderTabs();
        UpdateDisplay();
        UpdateHistory();
        saveCurrentSession();
        showFloatingMessage(i18n.updatedWorkFile);
        const popup = document.querySelector(".recent-files-section");
        if (popup) {
          popup.style.display = "none";
          setTimeout(() => popup.style.removeProperty("display"), 500);
        }
      } catch (err) {
        // recentFilesはstateの参照を直接操作
        const idx = recentFiles.indexOf(path);
        if (idx > -1) recentFiles.splice(idx, 1);
        localStorage.setItem("recentFiles", JSON.stringify(recentFiles));
        renderRecentFiles();
      }
    };
    el.oncontextmenu = (e) => {
      e.preventDefault();
      e.stopPropagation();
      const path = el.getAttribute("data-path");
      const idx = recentFiles.indexOf(path);
      if (idx > -1) recentFiles.splice(idx, 1);
      localStorage.setItem("recentFiles", JSON.stringify(recentFiles));
      renderRecentFiles();
      saveCurrentSession();
    };
  });
}

/**
 * タブリストを描画する（ドラッグ＆ドロップによる並び替え機能付き）
 */
export function renderTabs() {
  const list = document.getElementById("tabs-list");
  if (!list) return;
  list.innerHTML = "";

  // ポップアップ管理用
  let tooltip = null;

  tabs.forEach((tab) => {
    const el = document.createElement("div");
    // 基本クラス
    el.className = `tab-item ${tab.active ? "active" : ""}`;

    // 表示名の決定
    const fileName = tab.workFile
      ? tab.workFile.split(/[\\/]/).pop()
      : i18n?.selectedWorkFile || "New Tab";
    el.textContent = fileName;

    // --- 【追加】マウスホバーによるポップアップ表示 ---
    el.addEventListener("mouseenter", (e) => {
      if (tooltip) tooltip.remove();

      tooltip = document.createElement("div");
      tooltip.className = "tab-tooltip";

      const fullPath = tab.workFile || "No file selected";
      // 内容：タブ名(強調) + フルパス(コード風)
      tooltip.innerHTML = `<b>${fileName}</b><code>${fullPath}</code>`;

      document.body.appendChild(tooltip);

      // 表示位置の計算（タブの直下）
      const rect = el.getBoundingClientRect();
      tooltip.style.left = `${rect.left}px`;
      tooltip.style.top = `${rect.bottom + 5}px`;
    });

    // マウスが離れたらポップアップを消す
    el.addEventListener("mouseleave", () => {
      if (tooltip) {
        tooltip.remove();
        tooltip = null;
      }
    });

    // ドラッグ開始時やクリック時にもポップアップが残らないように制御
    el.addEventListener("mousedown", () => {
      if (tooltip) {
        tooltip.remove();
        tooltip = null;
      }
    });

    // --- ドラッグ＆ドロップ設定 ---
    el.draggable = true;
    el.dataset.id = tab.id;

    // ドラッグ開始
    el.ondragstart = (e) => {
      el.classList.add("dragging");
      // ドラッグ中のデータをセット（IDを渡す）
      e.dataTransfer.setData("text/plain", tab.id);
      e.dataTransfer.effectAllowed = "move";
    };

    // ドラッグ中（他のタブの上を通過時）
    el.ondragover = (e) => {
      e.preventDefault(); // ドロップを許可するために必須
      e.dataTransfer.dropEffect = "move";
      el.classList.add("drag-over");
    };

    // ドラッグが離れた時
    el.ondragleave = () => {
      el.classList.remove("drag-over");
    };

    // ドラッグ終了（成功・失敗問わず）
    el.ondragend = () => {
      el.classList.remove("dragging");
      // 全てのハイライトを消去
      list
        .querySelectorAll(".tab-item")
        .forEach((item) => item.classList.remove("drag-over"));
    };

    // ドロップされた時
    el.ondrop = (e) => {
      e.preventDefault();
      const draggedId = e.dataTransfer.getData("text/plain");
      const targetId = el.dataset.id;

      if (draggedId !== targetId) {
        // stateまたはactionsにある並び替え関数を呼び出す
        reorderTabs(draggedId, targetId);
      }
    };

    // --- 既存のクリックイベント ---
    el.onclick = () => switchTab(tab.id);

    // 右クリックで削除
    el.oncontextmenu = (e) => {
      e.preventDefault();
      if (tabs.length > 1) removeTab(tab.id);
    };

    list.appendChild(el);
  });
}

export function UpdateDisplay() {
  const tab = getActiveTab();
  if (!i18n || !tab) return;
  const fileEl = document.getElementById("selected-workfile");
  const dirEl = document.getElementById("selected-backupdir");
  if (fileEl)
    fileEl.textContent =
      (tab.workFile
        ? tab.workFile.split(/[\\/]/).pop()
        : i18n.selectedWorkFile) +
      (tab.workFile ? ` [${formatSize(tab.workFileSize)}]` : "");
  if (dirEl) dirEl.textContent = tab.backupDir || i18n.selectedBackupDir;

  const mode = document.querySelector(
    'input[name="backupMode"]:checked',
  )?.value;
  const isPass =
    mode === "archive" &&
    document.getElementById("archive-format")?.value === "zip-pass";
  const pwdArea = document.querySelector(".password-wrapper");
  if (pwdArea) {
    pwdArea.style.opacity = isPass ? "1" : "0.3";
    document.getElementById("archive-password").disabled = !isPass;
  }
  updateExecute();

  // Compact同期
  const cFileEl = document.getElementById("compact-selected-file");
  if (cFileEl)
    cFileEl.textContent = tab.workFile
      ? tab.workFile.split(/[\\/]/).pop()
      : i18n.selectedWorkFile || "No File Selected";
  const cSel = document.getElementById("compact-mode-select");
  if (cSel && mode) cSel.value = mode;
}

export async function UpdateHistory() {
  const tab = getActiveTab();
  const list = document.getElementById("diff-history-list");
  if (!list || !i18n) return;
  if (!tab?.workFile) {
    list.innerHTML = `<div class="info-msg">${i18n.selectFileFirst}</div>`;
    return;
  }

  try {
    const data = await GetBackupList(tab.workFile, tab.backupDir);
    if (!data || data.length === 0) {
      list.innerHTML = `<div class="info-msg">${i18n.noHistory}</div>`;
      return;
    }
    data.sort((a, b) => b.fileName.localeCompare(a.fileName));

    // --- 修正ポイント：勝手に tab の中身を書き換えない ---
    // 1. 本来の最新世代を取得
    const latestGenNumber = Math.max(
      ...data.map((item) => item.generation || 0),
    );

    // 2. 表示用のパスを決定する（tab.selectedTargetDir が優先）
    let activeDirPath = tab.selectedTargetDir;

    // もし tab.selectedTargetDir が完全に「空」の時だけ、最新を仮表示として採用する
    // ※ ここで tab.selectedTargetDir = ... と代入しないのがミソです

    if (!activeDirPath) {
      const first = data[0];
      // OSを問わず、最後のスラッシュ（/ または \）より前を抽出する
      activeDirPath = first.filePath.replace(/[\\/][^\\/]+$/, "");
    }

    const itemsHtml = await Promise.all(
      data.map(async (item) => {
        const note = await ReadTextFile(item.filePath + ".note").catch(
          () => "",
        );
        const isDiffFile = item.fileName.toLowerCase().endsWith(".diff");
        const isArchive = !isDiffFile && item.generation === 0;

        const itemDir =
          item.filePath.substring(0, item.filePath.lastIndexOf("/")) ||
          item.filePath.substring(0, item.filePath.lastIndexOf("\\"));

        let statusHtml = "";
        let genBadge = "";

        if (isArchive) {
          const archiveText = i18n.fullArchive || " Full Archive";
          statusHtml = `<div style="color:#2f8f5b; font-weight:bold;">${archiveText}</div>`;
          genBadge = `<span style="font-size:10px; color:#fff; background:#2f8f5b; padding:1px 4px; border-radius:3px; margin-left:5px;">Archive</span>`;
        } else {
          const currentGen = item.generation || 1;
          // activeDirPath（選択中パス or 仮の最新パス）と一致するか判定
          const isTarget = itemDir === activeDirPath;

          let statusColor = isTarget ? "#2f8f5b" : "#3B5998";
          let statusIcon = isTarget ? "✅" : "";
          let statusText = isTarget
            ? i18n.compatible || "書き込み先 (Active)"
            : i18n.genMismatch || "別世代 (クリックで切替)";

          const genLabel = i18n.generationLabel || "Gen";
          const currentLabel = isTarget
            ? ` <span style="font-size:9px; opacity:0.9;">(Target)</span>`
            : "";
          const badgeStyle = `font-size:10px; color:#fff; background:${statusColor}; padding:1px 4px; border-radius:3px; margin-left:5px; ${isTarget ? "outline: 2px solid #2f8f5b; outline-offset: 1px;" : ""} cursor:pointer;`;

          statusHtml = `<div style="color:${statusColor}; font-weight:bold;">${statusIcon} ${statusText}</div>
                      <div style="font-size:11px; color:#666;">${genLabel}: ${currentGen} ${isTarget ? "★" : ""}</div>`;

          genBadge = `<span class="gen-selector-badge" data-dir="${itemDir}" style="${badgeStyle}">${genLabel}.${currentGen}${currentLabel}</span>`;
        }

        const popupContent = `${statusHtml}<hr style="border:0; border-top:1px solid #eee; margin:5px 0;"><strong>Path:</strong> ${item.filePath}${note ? `<br><hr style="border:0; border-top:1px dashed #ccc; margin:5px 0;"><strong>${i18n.backupMemo}:</strong> ${note}` : ""}`;

        return `<div class="diff-item" style="${itemDir === activeDirPath ? "border-left: 4px solid #2f8f5b; background: #f0fff4;" : ""}">
          <div style="display:flex; align-items:center; width:100%;">
            <label style="display:flex; align-items:center; cursor:pointer; flex:1; min-width:0;">
              <input type="checkbox" class="diff-checkbox" value="${item.filePath}" style="margin-right:10px;">
              <div style="display:flex; flex-direction:column; flex:1; min-width:0;">
                <span class="diff-name" data-hover-content="${encodeURIComponent(popupContent)}" style="font-weight:bold; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;">
                  ${item.fileName} ${genBadge} <span style="font-size:10px; color:#3B5998;">(${formatSize(item.fileSize)})</span>
                </span>
                <span style="font-size:10px; color:#888;">${item.timestamp}</span>
                ${note ? `<div style="font-size:10px; color:#2f8f5b; font-style:italic; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;"> ${note}</div>` : ""}
              </div>
            </label>
            <button class="note-btn" data-path="${item.filePath}" style="background:none; border:none; cursor:pointer; font-size:14px; padding:4px;"></button>
          </div>
        </div>`;
      }),
    );

    list.innerHTML = itemsHtml.join("");

    // --- イベントリスナーの追加 ---
    list.querySelectorAll(".gen-selector-badge").forEach((el) => {
      el.addEventListener("click", (e) => {
        e.preventDefault();
        e.stopPropagation();
        // ユーザーの意思を tab.selectedTargetDir に叩き込む
        tab.selectedTargetDir = el.getAttribute("data-dir");
        saveCurrentSession();
        UpdateHistory();
      });
    });

    // --- メモボタンの修正版リスナー ---
    list.querySelectorAll(".note-btn").forEach((btn) => {
      btn.onclick = async (e) => {
        e.preventDefault();
        e.stopPropagation();

        const path = btn.getAttribute("data-path");
        const notePath = path + ".note";

        // ファイルから現在のメモを読み込み
        const currentNote = await ReadTextFile(notePath).catch(() => "");

        // ダイアログを表示（冒頭でインポート済みの関数）
        showMemoDialog(currentNote, async (newText) => {
          try {
            await WriteTextFile(notePath, newText);
            showFloatingMessage(i18n.memoSaved);
            UpdateHistory(); // 再描画
          } catch (err) {
            console.error(err);
            showFloatingError(i18n.memoSaveError);
          }
        });
      };
    });

    setupHistoryPopups();
  } catch (err) {
    console.error(err);
    list.innerHTML = `<div class="info-msg" style="color:red;">Error: ${err.message || "loading history"}</div>`;
  }
}

function setupHistoryPopups() {
  // IDを history-tooltip に変更
  const tooltip =
    document.getElementById("history-tooltip") || createTooltipElement();
  const targets = document.querySelectorAll(".diff-name");

  targets.forEach((target) => {
    target.onmouseenter = (e) => {
      const content = decodeURIComponent(
        target.getAttribute("data-hover-content"),
      );
      tooltip.innerHTML = content;
      tooltip.classList.remove("hidden");

      // 位置計算（ロジックは維持）
      const rect = target.getBoundingClientRect();
      tooltip.style.left = `${rect.left}px`;
      tooltip.style.top = `${rect.bottom + 5}px`;
    };

    target.onmouseleave = () => {
      tooltip.classList.add("hidden");
    };
  });
}

function createTooltipElement() {
  const el = document.createElement("div");
  // IDとクラス名を history-tooltip に変更
  el.id = "history-tooltip";
  el.className = "history-tooltip hidden";
  document.body.appendChild(el);
  return el;
}

export function toggleProgress(show, text = "") {
  const displayMsg = text || (i18n ? i18n.processingMsg : "Processing...");
  const container = document.getElementById("progress-container");
  const bar = document.getElementById("progress-bar");
  const status = document.getElementById("progress-status");
  const btn = document.getElementById("execute-backup-btn");
  const cBar = document.getElementById("compact-progress-bar");
  const cSts = document.getElementById("compact-status-label");
  const cBtn = document.getElementById("compact-execute-btn");

  if (show) {
    if (container) container.style.display = "block";
    if (status) {
      status.style.display = "block";
      status.textContent = displayMsg;
    }
    if (bar) bar.style.width = "0%";
    if (btn) btn.disabled = true;
    if (cSts) cSts.textContent = displayMsg;
    if (cBar) cBar.style.width = "0%";
    if (cBtn) cBtn.disabled = true;
  } else {
    if (bar) bar.style.width = "100%";
    if (cBar) cBar.style.width = "100%";
    setTimeout(() => {
      if (container) container.style.display = "none";
      if (status) status.style.display = "none";
      if (btn) btn.disabled = false;
      if (cSts) cSts.textContent = "Ready";
      if (cBar) cBar.style.width = "0%";
      if (cBtn) cBtn.disabled = false;
    }, 500);
  }
}
