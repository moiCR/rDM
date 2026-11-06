# rDM

## Project

- **rDM** is a modern, high-performance Internet Download Manager (IDM) built with Rust, Tokio, and GPUI (`gpui-pre`).
- Prioritize maintainability, readability, and correctness over clever optimizations.
- Preserve the project's clean, responsive, and minimal UI philosophy.

## Rust

- Write idiomatic Rust (Edition 2024).
- Never use `unwrap()`, `expect()`, or other panicking APIs in production code.
- Propagate errors using `Result`, `thiserror`, and `?`.
- Avoid unnecessary cloning and allocations; use `Arc`, `Bytes`, and atomic primitives (`AtomicU64`, etc.) where appropriate.
- Prefer descriptive names over abbreviations.
- Keep functions focused on a single responsibility.
- Run `cargo fmt` before finishing changes.
- Keep `cargo clippy` free of warnings.
- Don't write comments in the code.

## Architecture & Runtime Separation

- Prefer extending existing modules before creating new ones.
- Do not create new crates or modules unless there is a clear architectural reason.
- Keep related functionality together.
- Avoid introducing traits unless multiple implementations are expected.
- Favor composition over unnecessary abstractions.
- **Tokio vs. GPUI Executor**:
  - GPUI worker threads (`Worker-*`, `smol-*`) do NOT run a Tokio reactor.
  - All network interactions (`reqwest`), multi-connection download workers, chunked file I/O (`DiskManager`), and background IPC/extension listeners MUST run inside Tokio's runtime via `tokio::spawn` or with an entered runtime handle (`let _guard = core.tokio_handle().enter();`).
  - Never instantiate or await `reqwest::Client` streams or Tokio I/O primitives directly inside bare GPUI tasks (`cx.spawn` / `background_executor().spawn`).
  - Communicate from background Tokio download workers to GPUI strictly via asynchronous channels (`tokio::sync::mpsc`, `watch`, or `broadcast`) or lock-free atomics (`AtomicU64`) polled at controlled UI intervals, ingesting state updates in the UI via `window_context.update` or `entity.update`.

## GPUI & Rendering Rules

- **`render()` Must Be Pure & Side-Effect Free**:
  - Keep rendering declarative. Never run business logic, download task spawning, disk checks, or state mutations inside `render()`.
  - PROHIBITED inside `render()`: `window.focus()`, `window.activate_window()`, `window.refresh()`, and `cx.notify()`.
  - Focus must ONLY be requested upon explicit window open, modal transitions, or view changes (`if self.last_rendered_mode != Some(self.mode)`), never during paint passes.
- **Window State & Surface Commits**:
  - Never call window/surface setters inside `render()` without a strict cache diff:
    ```rust
    if self.cached_state != Some(desired_state) {
        self.cached_state = Some(desired_state);
        window.set_state(desired_state);
    }
    ```
- **Element Reconciliation & Dynamic Loops**:
  - Any interactive element with `.hover(...)`, `.on_click(...)`, or event handlers created inside an iterator or loop (such as download task rows, segment progress bars, or category filters) MUST be given a unique, stable ID (e.g., `.id(("download-task", task.id))`). Failing to do so invalidates the hit-test tree every frame and causes severe frame drops during active downloads.
  - Avoid dynamic heap churn (`Vec<AnyElement>` boxing) inside hot render passes when declarative chaining is possible.
- **Dimension Oscillation Prevention**:
  - Computed dialog, inspector, and popup panel heights (`compute_*_panel_height()`) must match the actual rendered layout height to prevent layout oscillation loops.
- Store application state in entities, not stateless UI components.
- Never block the UI thread with synchronous file system or network calls.
- Background work belongs in background tasks (`rdm-core`).
- Reuse UI components whenever practical.

## Code Style

- Comments should explain **why**, never **what** (avoid comments unless strictly necessary).
- Remove dead code instead of commenting it out.
- Avoid TODOs unless explicitly requested.
- Do not make unrelated refactors while implementing a feature.

## Scope

- Only implement what was requested.
- Do not add features, configuration options, or dependencies unless explicitly required.
- Avoid speculative abstractions.

## Codebase Structure

```
crates/
├── app/          Main GUI binary (GPUI), modules, widgets, shared UI components, theme, and embedded assets (planned)
├── cli/          Command-line interface binary for headless/terminal downloads
├── core/         Download engine (`Engine`), task orchestration (`Task`), disk I/O (`DiskManager`), and errors
└── config/       Configuration management (`ConfigManager`, `EngineConfig`)
```

## App Structure (Modules, Widgets, Components & Assets)

Each module in `crates/app` should have its own widgets separated from module state logic. Do not cram everything into the module file. Shared UI components and assets live directly inside `crates/app`:

## Multiple Instances

Avoid creating multiple instances of core services or managers. `Core` (`Engine`, `DiskManager`, `ConfigManager`) must be instantiated once and shared globally (`Arc` / GPUI global `AppState`).

## Important Conventions

- **Async Timers in UI vs. Engine**:
  - In GPUI contexts, use `cx.background_executor().timer(duration).await` (NOT `tokio::time::sleep`).
  - In `rdm-core` background tasks running on Tokio, use `tokio::time::sleep(duration).await` or `tokio::time::interval`.
- **Progress Reporting & UI Refresh Floor**:
  - Never trigger `cx.notify()` on every network chunk (`bytes::Bytes`) received from `reqwest`. High-speed downloads receive thousands of chunks per second and will saturate the UI thread.
  - Accumulate downloaded bytes via `AtomicU64` (`Ordering::Relaxed`) in `Task` and coalesce UI progress updates to a bounded tick rate (e.g., 16ms–50ms floor, only notifying when values change).
- **File System & Disk I/O Isolation**:
  - UI interactions, modal openings, and search/filter queries must NEVER trigger synchronous disk scans (`std::fs::metadata`, directory listings, or file pre-allocation).
  - All file pre-allocation, chunk flushing, segment merging, and disk space checks belong strictly in `rdm-core::disk::DiskManager` on Tokio async/blocking pools.
- **Icons**: SVGs in `crates/app/assets/icons/`, loaded via the GPUI asset system.
- **Styling**: Pure GPUI builder patterns (no CSS).
- **Async Runtime Bridging & OS Integration**:
  - Opening downloaded files or revealing folders in the OS file manager (`std::process::Command::spawn()`) is non-blocking; never wrap immediate OS process spawns in nested cross-runtime locks that could deadlock the UI when idle.
- **Resilient Download Workers & Cancel Safety**:
  - Multi-connection segment workers must handle transient network drops with exponential backoff and support clean pause/resume via `CancellationToken` or abort handles without corrupting partial files on disk.
  - Never wrap non-cancel-safe stream reads inside destructive `tokio::select!` branches without preserving buffered stream state.
- **Clipboard Integration**:
  - Use GPUI's native clipboard APIs (`cx.write_to_clipboard` / `cx.read_from_clipboard`) for URL capture and copy actions. Validate URLs cleanly before auto-populating add-download modals.

## Notes

- Before exploring the repository structure, consult `codebase-memory-mcp` if available.
- Check that the index belongs to the current project and is up to date.
- Use the graph to locate relevant symbols, dependencies, and files.
- Verify the source code before modifying it.
- If the graph is unavailable or its coverage is insufficient, inspect the files directly and disclose that limitation.
- Use caveman skill.
