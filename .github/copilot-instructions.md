# iLauncher AI Coding Instructions

## Project Overview

iLauncher is a **Tauri 2 + React 19 + Rust** cross-platform application launcher with lightning-fast MFT file search on Windows. Inspired by Wox/Raycast, it features plugin architecture, clipboard history, and real-time file indexing.

**Tech Stack**: Tauri 2, React 19, TypeScript, Rust, Zustand, TailwindCSS, Vite

## Architecture

### Hybrid Frontend-Backend Structure

```
src/               → React frontend (TypeScript)
src-tauri/src/     → Rust backend (Tauri commands)
├─ commands/       → Tauri command handlers (query, execute, config)
├─ plugin/         → Plugin system (15+ built-in plugins)
├─ mft_scanner/    → Windows MFT file indexing (admin rights required)
├─ core/           → Search engine, ranking, fuzzy matching
├─ storage/        → SQLite/JSON config persistence
├─ clipboard.rs    → Clipboard history monitoring
└─ hotkey/         → Global hotkey registration (Alt+Space)
```

### Key Data Flows

1. **Search Query**: `SearchBox.tsx` → `invoke("query")` → `commands::query()` → `PluginManager::query()` → parallel plugin execution → fuzzy match + ranking → return results
2. **MFT Indexing** (Windows): UI process spawns elevated `--mft-service` subprocess → USN journal monitoring → RoaringBitmap + FST compression → writes to `AppData\Local\iLauncher\mft_databases\`
3. **Configuration**: `useConfigStore` (Zustand) ↔ `invoke("get_config"/"save_config")` ↔ `storage/StorageManager` → `config.json`

### Plugin System

**Pattern**: Every plugin implements `Plugin` trait with `metadata()`, `query()`, `execute()`.

**Examples**:
- `file_search.rs`: Uses MFT on Windows (fallback to BFS), pinyin search via `pinyin-pro`
- `calculator.rs`: Math expression evaluation via `meval`
- `browser.rs`: Parses Chrome/Edge bookmarks (JSON) + history (SQLite copy)
- `process.rs`: Lists running processes via `sysinfo`, supports `kill` action
- `translator.rs`: Local dictionary + Google Translate API fallback

**Plugin Location**: `src-tauri/src/plugin/*.rs`, registered in `mod.rs::PluginManager::new()`

## Critical Developer Workflows

### Running Development Mode

```powershell
# Frontend only (Vite HMR)
bun run dev

# Full stack (Tauri + Vite)
bun tauri dev

# MFT Scanner debug (skip initial scan)
cargo run --bin ilauncher -- --mft-service --skip-scan --ui-pid 1234
```

### Building for Production

```powershell
# Compile and bundle
bun tauri build

# Generate signed release artifacts
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content ~/.tauri/ilauncher.key -Raw)
bun tauri build
```

### Testing MFT Indexing

MFT requires **admin elevation**. UI uses `ShellExecuteW` with `"runas"` to spawn `--mft-service` subprocess.

```rust
// Check MFT status
invoke("get_mft_status")  // Returns { ready: bool, drives: ["C", "D"] }

// Toggle MFT on/off
invoke("toggle_mft", { enabled: true })
```

**Index files** (per drive): `C_index.fst`, `C_bitmaps.dat`, `C_paths.dat`, `C.ready` (marker file with PID)

### Plugin Development Template

```rust
pub struct MyPlugin {
    metadata: PluginMetadata,
}

impl Plugin for MyPlugin {
    fn metadata(&self) -> &PluginMetadata { &self.metadata }
    
    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        if !ctx.query.starts_with("trigger") { return Ok(vec![]); }
        // Build results with actions
        Ok(vec![QueryResult { /* ... */ }])
    }
    
    async fn execute(&self, result_id: &str, action_id: &str) -> Result<()> {
        match action_id {
            "open" => { /* open file */ },
            "copy" => { /* copy to clipboard */ },
            _ => {}
        }
        Ok(())
    }
}
```

## Project-Specific Conventions

### Naming Patterns

- **Tauri Commands**: Snake case (`get_config`, `execute_action`) in `commands/mod.rs`
- **React Components**: PascalCase (`SearchBox.tsx`, `Settings.tsx`) in `src/components/`
- **Stores**: `use*Store` pattern (Zustand) in `src/store/`
- **Plugin IDs**: Snake case (`file_search`, `git_projects`)

### State Management

- **Global State**: Zustand stores (`useAppStore`, `useConfigStore`, `themeStore`)
- **Config Persistence**: All changes via `useConfigStore.saveConfig()` → auto-saves to backend
- **Window Lifecycle**: Views switch window size (`VIEW_CONFIGS` in `App.tsx`), blur in search mode auto-hides

### Styling & Themes

- **TailwindCSS**: Utility-first styling, custom CSS variables for themes
- **Theme System**: 13 built-in themes in `theme.ts`, applied via CSS variables (`--color-primary`, `--color-surface`)
- **Dynamic Theming**: `applyTheme()` updates CSS vars, configs stored in `appearance.theme` + optional `custom_theme`

### Error Handling

- **Rust**: `anyhow::Result<T>` for most functions, `thiserror` for custom errors
- **Frontend**: `try/catch` + `useToast` for user notifications
- **Logging**: `tracing` crate → writes to `AppData\Local\iLauncher\logs\ilauncher.log` + `mft_service.log`

## Integration Points

### Tauri IPC

**Commands**: Defined in `lib.rs::tauri::generate_handler![]`, implemented in `commands/*.rs`

```typescript
// Frontend invocation
import { invoke } from "@tauri-apps/api/core";
const results = await invoke<QueryResult[]>("query", { query: "test" });
```

### Window Management

- **Show/Hide**: `commands::show_app()` / `hide_app()` → centers window + focuses
- **Auto-hide**: Blur event in search view triggers `hide_app()` (see `App.tsx`)
- **Tray Icon**: System tray with "Show", "Settings", "Quit" menu items

### File System Paths

- **Config**: `%LOCALAPPDATA%\iLauncher\config\config.json`
- **MFT Index**: `%LOCALAPPDATA%\iLauncher\mft_databases\`
- **Logs**: `%LOCALAPPDATA%\iLauncher\logs\`
- **Icons Cache**: `%TEMP%\ilauncher_icons\` (Windows file icons via `SHGetFileInfoW`)

### External Dependencies

- **MFT Scanning**: Windows-only, requires `windows` crate (v0.58), uses USN journal + RoaringBitmap compression
- **Clipboard**: Cross-platform via `arboard`, SQLite history in `clipboard.db`
- **Hotkey**: Global via `global-hotkey` crate, registered on app startup

## Build & Release

### CI/CD Pipeline (`.github/workflows/release.yml`)

**Trigger**: Push tag `v*` → auto-builds Windows/macOS/Linux → creates GitHub Release

**Steps**:
1. Extract version from tag (`v0.1.0` → `0.1.0`)
2. Update `package.json`, `tauri.conf.json`, `Cargo.toml`
3. Run `tauri-action` (builds + signs + uploads)
4. Generate `latest.json` for auto-updater

**Secrets Required**:
- `TAURI_SIGNING_PRIVATE_KEY`: Generated via `bunx tauri signer generate`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: Optional passphrase

### Versioning

Use semantic versioning (`v1.2.3`). Synchronize across:
- `package.json` → `"version": "0.1.0"`
- `src-tauri/tauri.conf.json` → `"version": "0.1.0"`
- `src-tauri/Cargo.toml` → `version = "0.1.0"`

## Common Patterns

### Adding a New Plugin

1. Create `src-tauri/src/plugin/my_plugin.rs`
2. Implement `Plugin` trait with trigger words in metadata
3. Register in `src-tauri/src/plugin/mod.rs::PluginManager::new()`
4. Test via search query matching trigger word

### Adding a New Tauri Command

1. Define function in `src-tauri/src/commands/*.rs`
2. Add to `lib.rs::tauri::generate_handler![]`
3. Frontend: `invoke("command_name", { args })`

### Updating Configuration Schema

1. Update `AppConfig` interface in `src/types/index.ts`
2. Update Rust struct in `src-tauri/src/storage/config.rs`
3. Handle migration in `StorageManager::load_config()` if breaking change

### Custom Themes

Users can create themes in Settings → Theme Editor. Custom themes stored in `config.appearance.custom_theme`.

**Structure**: See `Theme` interface in `src/theme.ts` (colors, appearance, font)

## Important Notes

- **MFT Service**: Runs as separate elevated process, auto-exits when UI closes (monitors UI PID)
- **Pinyin Search**: Chinese character support via `pinyin-pro` on frontend (converted before backend search)
- **Icon Caching**: Windows file icons extracted once per extension, cached in temp dir
- **Auto-update**: Uses Tauri updater plugin, checks GitHub releases on startup (5s delay)
- **Cross-platform**: Full Linux/macOS support, but MFT Windows-only (fallback to `walkdir` BFS)

## References

- [Tauri 2 Docs](https://v2.tauri.app/)
- [Plugin Development](../PLUGINS_SUMMARY.md)
- [Theme System](../docs/THEME_CONFIG_INTEGRATION.md)
- [Icon Feature](../docs/ICON_FEATURE.md)
- [Release Process](workflows/README.md)
