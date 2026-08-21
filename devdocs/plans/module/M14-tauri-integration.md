# M14: Tauri Shell Integration

**Layer**: 6 — Desktop Shell
**Depends on**: All M00-M13 modules must be stable
**Effort**: 1.5 weeks

## Purpose

Wrap the Axum backend + Vue frontend in a Tauri desktop shell, creating
a single native binary. The Axum server runs inside the Tauri Rust
process. HTTP is the primary API channel (existing code unchanged); Tauri
IPC is added for high-bandwidth data paths (images, point clouds).

## Deliverables

### D1: Tauri project scaffold

- Run `cargo tauri init` in the workspace
- Configure `tauri.conf.json`:
  - `build.devPath`: `../frontend/dist` (Vite build output)
  - `build.distDir`: `../frontend/dist`
  - `build.beforeDevCommand`: `npm --prefix ../frontend run dev`
  - `build.beforeBuildCommand`: `npm --prefix ../frontend run build`
- Tauri Rust entry point (`src-tauri/src/main.rs`) starts Axum server in a tokio task:
  ```rust
  fn main() {
      tauri::Builder::default()
          .setup(|app| {
              let state = Arc::new(AppState { ... });
              tokio::spawn(async {
                  axum::Server::bind(&"127.0.0.1:3001".parse().unwrap())
                      .serve(build_router(state).into_make_service())
                      .await
              });
              Ok(())
          })
          .run(tauri::generate_context!())
          .unwrap();
  }
  ```

### D2: IPC channels for high-bandwidth data

Tauri commands for large data (bypass JSON serialization when beneficial):

```rust
#[tauri::command]
async fn get_point_cloud(timestamp: u64) -> Result<Vec<u8>, String> {
    // Return raw Arrow bytes, decoded in WebView
}

#[tauri::command]
async fn get_camera_image(timestamp: u64, camera: String) -> Result<Vec<u8>, String> {
    // Return raw image bytes (JPEG/PNG)
}

#[tauri::command]
async fn subscribe_topic(topic: String) -> Result<(), String> {
    // Register for streaming updates via Tauri events
}
```

Frontend distinguishes:
- Regular API calls → `fetch("http://127.0.0.1:3001/api/...")` (unchanged)
- Large data → `window.__TAURI__.invoke("get_point_cloud", { timestamp })`
- Streaming → `window.__TAURI__.event.listen("topic-update", callback)`

### D3: Single binary build

- `cargo tauri build` produces a single executable
- Bundled assets: Vue frontend (Vite build), models/ directory
- No external dependencies except `dora` CLI on PATH
- Target platforms: Linux (primary), macOS (secondary if time permits)
- Binary size target: <50MB compressed

### D4: Graceful fallback

- `window.__TAURI__` check: if running in browser (dev mode), fall back to HTTP-only
- Dev workflow unchanged: `npm run dev` + `cargo run` still works
- Tauri mode detected automatically: `if ('__TAURI__' in window) { ... }`

### D5: Menu bar + system tray (optional polish)

If time permits (not required for acceptance):
- File menu: Open .drec, Open Dataset, Quit
- View menu: Toggle sidebar, Toggle dark mode
- Help menu: About, Documentation link
- System tray icon with quick status (coordinator connected/disconnected)

## Acceptance Criteria

- [ ] `cargo tauri build` produces a working Linux binary
- [ ] Running the binary starts Axum server + opens Tauri window
- [ ] All existing HTTP API endpoints work (full regression test)
- [ ] Running in browser (dev mode) still works without Tauri
- [ ] IPC `get_point_cloud` returns correct data for a known timestamp
- [ ] Binary size <50MB compressed
- [ ] Application closes cleanly: stops Axum, kills child processes, exits
- [ ] No network dependency: all features work offline

## Exposed Interfaces

This module is the top of the stack. It exposes no programmatic interfaces
to other modules — it consumes everything below it.

The user-facing interface is:
- Desktop application window with menu bar
- WebView rendering the Vue 3 frontend
- System tray icon (optional)
