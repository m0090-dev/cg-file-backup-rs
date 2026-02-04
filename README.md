# WorkBackupTool

A simple, lightweight backup utility built with Tauri (Rust + Svelte), specifically designed for managing 2DCG project files and work-in-progress versions.

This tool was created by the author for personal use to ensure quick and reliable versioning during creative workflows.


## ✅ Implementation Status
The following is the current progress of features and planned updates.

---

### Core Backup Functions
- [x] **Full Copy**: Simple duplication of workspace files.  
- [x] **Archive**: Support for zip and tar.gz formats.  
- [x] **Encrypted Archive**: ZIP with password protection (Pending).  
- [x] **Differential (hdiff)**: Supports zstd / lzma2 / none compression methods.    
- [x] **Generation Management**: Automatically creates a new ".base" (full copy baseline) and updates the backup destination when the diff size exceeds a threshold.  

---

### User Interface (UI/UX)
- [x] **Tab Management**: Manage multiple projects simultaneously. Supports reordering, right-click to delete, and path/filename display on focus.  
- [x] **Backup History**: Browse history with metadata display (path, notes, and generation info) on focus.  
- [x] **Note System**: Create text memos attached to backup entries (stored as ".note" files) via the Note button.  
- [x] **Compact Mode**: Locks the window to a minimal size.  
- [x] **Tray Mode**: Minimizes the app to the system tray and hides the window.  
- [x] **Window Options**: "Always on Top" and "Restore Previous State" functionality.  
- [x] **Multi-language**: Support for English and Japanese.  

## 🔎 Key Features

- **Streamlined UI**: A single-window interface focused on "Backup" and "Restore."
- **Backup Modes**:
  - **Full Copy**: Creates a standard mirror of your files.
  - **Archive**: Compresses data into ZIP/TAR formats with optional password protection.
  - **Incremental (Smart)**: Saves disk space by backing up only modified parts (using Hdiff, etc.).
- **Quick Restore**: Browse your backup history and revert to a specific point in time with one click.

## 🚀 How to Use

1. **Target**: Select the file or folder you want to back up.
2. **Location**: Set the destination folder where backups will be stored.
3. **Execute**: Choose your preferred backup mode and click "Execute" (実行).
4. **Restore**: Select a previous version from the history list and click "Restore to selected point" (選択した時点へ復元).

## 🛠 For Developers

This application is built using [Tauri]().

### Prerequisites

- Rust
- Node.js
- Tauri CLI

### Commands

```bash
# Run in development mode
npm run tauri:dev

# Build the application
npm run tauri:build
```

# 📦 Distribution Notes

If you are using the pre-compiled version, please note:

- **External Dependencies**: This tool includes `hdiff-bin` to handle differential backups and compression. Do not delete these directories, as they are essential for the tool's core functionality.
- **Licenses**: This software uses several open-source libraries. You can find the list of used libraries in `CREDITS.md` and their full license texts in the `licenses/` directory.


## License

This project is licensed under the MIT License - see the [LICENSE.md](LICENSE.md) file for details.
Copyright (c) 2024-2026 m0090-dev
For a complete list of third-party licenses and credits, please refer to [CREDITS.md](CREDITS.md).
