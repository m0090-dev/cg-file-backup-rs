import {
  WriteTextFile,
  ReadTextFile,
  GetConfigDir,
  GetRestorePreviousState,
} from "./tauri_exports";

// --- 状態管理 ---
export let i18n = null;
export let tabs = [
  {
    id: Date.now(),
    workFile: "",
    workFileSize: 0,
    backupDir: "",
    active: true,
    selectedTargetDir: "",
    backupMode: "diff",
    compressMode: "zstd",
    diffAlgo: "hdiff",
  },
];
export let recentFiles = JSON.parse(
  localStorage.getItem("recentFiles") || "[]",
);
export const MAX_RECENT_COUNT = 5;
export const SESSION_FILE_NAME = "session.json";

// i18nを更新するためのセッター関数を追加
export function setI18N(data) {
  i18n = data;
}

// --- ヘルパー ---
export function getActiveTab() {
  return tabs.find((t) => t.active);
}

export function addToRecentFiles(path) {
  if (!path) return;
  recentFiles = [path, ...recentFiles.filter((p) => p !== path)].slice(
    0,
    MAX_RECENT_COUNT,
  );
  localStorage.setItem("recentFiles", JSON.stringify(recentFiles));
  // renderRecentFiles() はUI層にあるため、ここでは状態更新のみ
}

// --- セッション保存・復元ロジック ---
export async function saveCurrentSession() {
  try {
    const shouldRestore = await GetRestorePreviousState();
    if (!shouldRestore) return;
    const configDir = await GetConfigDir();
    const sessionPath = configDir + "/" + SESSION_FILE_NAME;
    const data = JSON.stringify({ tabs, recentFiles });
    await WriteTextFile(sessionPath, data);
  } catch (err) {
    console.error("Save session failed:", err);
  }
}

export async function restoreSession() {
  try {
    const shouldRestore = await GetRestorePreviousState();
    if (!shouldRestore) return;
    const configDir = await GetConfigDir();
    const sessionPath = configDir + "/" + SESSION_FILE_NAME;
    const content = await ReadTextFile(sessionPath);
    if (content) {
      const saved = JSON.parse(content);
      if (saved.tabs && saved.tabs.length > 0) {
        // 配列の中身を入れ替える（参照を維持するため）
        tabs.splice(0, tabs.length, ...saved.tabs);
      }
      if (saved.recentFiles) {
        recentFiles.splice(0, recentFiles.length, ...saved.recentFiles);
        localStorage.setItem("recentFiles", JSON.stringify(recentFiles));
      }
    }
  } catch (err) {
    console.log("No session to restore.");
  }
}

export function formatSize(bytes) {
  if (!bytes) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
}
