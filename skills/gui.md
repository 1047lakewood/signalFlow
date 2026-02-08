# signalFlow — GUI (Tauri) Design Doc

## Architecture

```
signalFlow/
├── src/                   # Core Rust library (signal_flow crate)
├── src/main.rs            # CLI binary
├── src-tauri/             # Tauri v2 backend (signalflow-gui crate)
│   ├── Cargo.toml         # Depends on signal_flow via path = ".."
│   ├── build.rs           # tauri_build::build()
│   ├── tauri.conf.json    # Tauri config (window, build, bundle)
│   ├── capabilities/      # Tauri v2 permission capabilities
│   │   └── default.json   # Core permissions for main window
│   ├── icons/
│   │   └── icon.ico       # App icon (placeholder)
│   └── src/
│       └── main.rs        # Tauri app entry, AppState, IPC commands
├── gui/                   # React + TypeScript frontend
│   ├── package.json       # Dependencies (@tauri-apps/api, react, vite)
│   ├── vite.config.ts     # Vite dev server on port 1420
│   ├── tsconfig.json      # Strict TS config
│   ├── index.html         # Entry HTML
│   └── src/
│       ├── main.tsx       # React root mount
│       ├── App.tsx        # Main app component
│       └── styles.css     # Dark-first theme
└── Cargo.toml             # Workspace root (members: src-tauri)
```

## Workspace Setup (DONE)

- Root `Cargo.toml` defines `[workspace]` with `members = ["src-tauri"]`
- Core library (`signal_flow`) remains at root, unchanged
- Tauri binary (`signalflow-gui`) in `src-tauri/` depends on `signal_flow` via path

## Tauri Backend (DONE)

- `AppState` wraps `Engine` in `Mutex` for thread-safe IPC access
- `Engine::load()` called at startup to restore persisted state
- Initial IPC command: `get_status` — returns engine summary string
- Tauri v2 with capabilities-based permissions (`core:default`)

## Frontend (DONE)

- React 19 + TypeScript + Vite 6
- Dark-first theme (CSS custom properties): `--bg-primary: #1a1a2e`, `--highlight: #e94560`
- `@tauri-apps/api` for IPC via `invoke()`
- Dev server: `http://localhost:1420`
- Build output: `gui/dist/`

## Running

- Dev mode: `cd src-tauri && cargo tauri dev` (starts Vite + Tauri together)
- Build: `cd src-tauri && cargo tauri build`
- Frontend only: `cd gui && npm run dev`

## IPC Commands

| Command      | Args | Returns  | Status |
|-------------|------|----------|--------|
| `get_status` | none | `String` | DONE   |

## Next Steps

- [ ] IPC bridge — expose all core engine functions as Tauri commands
- [ ] Main playlist view — track list with columns
- [ ] Transport controls — Play, Stop, Skip buttons
