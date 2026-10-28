# Handover: native GPUI rewrite (branch `feat/native-gpui`)

Work in progress. Delete this file when the rewrite lands.

## Decisions

- UI framework: GPUI through `gpui-kit` 0.8.1 (pinned). Evidence and alternatives: [docs/decisions/0001-ui-framework.md](docs/decisions/0001-ui-framework.md).
- Web-library replacements (RaTeX for math, `mermaid-rs-renderer`, `hayro` for PDF, `alacritty_terminal`, HTML export instead of PDF export): [docs/decisions/0002-web-library-replacements.md](docs/decisions/0002-web-library-replacements.md).
- Hexagonal workspace, rules in [AGENTS.md](AGENTS.md), architecture in [docs/architecture.md](docs/architecture.md), design rules in [DESIGN.md](DESIGN.md).
- No comments anywhere, no unsafe, Google Rust style, strict lints, all enforced (`cargo xtask check`, clippy config in `Cargo.toml` / `clippy.toml`).
- Tooling: `mise` (`mise.toml`: tools + tasks), `rust-toolchain.toml` (1.98.1), nightly `nightly-2026-10-27` only for rustfmt, `lefthook.yml` hooks, CI in `.github/workflows/ci.yml` (6 targets).
- On-disk formats, crypto and S3 layout are byte-compatible with Tauri 0.1.30 (contract in docs/architecture.md).

## State

Done and green (fmt, clippy `-D warnings`, tests, `cargo deny`, xtask) for the whole core:

- shared: `margin-kernel`, `margin-crypto`, `margin-markdown` (full parser with marker ranges, tables, frontmatter, HTML export renderer), `margin-retry`, `margin-dates`
- features: vaults, files, trash, attachments, search, history, settings, sync (with retry), ask, terminals
- adapters: fs (+ watcher, reveal/open), sqlite (FTS5), s3 (own SigV4 client), llm (OpenAI/Anthropic), pty
- testing: `margin-test-support` (fakes, conformance suites, S3 stub, sync round trip)

Started, not compiled yet:

- `margin-theme` (palette from tokens.css, GPUI Kit theme projection, bundled Inter/JetBrains Mono)
- `margin-i18n` (messages/{en,it}.json, plurals `_one/_other`)
- `margin-workbench` (UI vocabulary: VaultServices, actions, WorkbenchEvent, TabContent, run_blocking/run_async, Platform, SyncFactory)
- `margin-desktop`: `app.rs`, `network.rs` (Tokio runtime + Timer), `container.rs` (composition root), `vault_reader.rs`, `history_snapshots.rs`, `window.rs`. Missing: `workspace.rs` (shell).

Not started: UI crates `file-tree`, `editor` (live preview), `spotlight`, `terminal`, `viewers`, `canvas`, `panels`, `onboarding`; updater; packaging (`cargo packager` config, `release.yml` still the Tauri one).

## Next steps

1. Compile the UI foundation once: `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=<repo>/target/ui cargo clippy -p margin-theme -p margin-i18n -p margin-workbench --all-targets -- -D warnings` and fix API mismatches with gpui-kit (first GPUI build takes ~15 min).
2. Build the UI crates against the contract below (one shared target dir `target/ui`, `CARGO_BUILD_JOBS=2`: the machine is slow).
3. Write the desktop `workspace.rs` shell (DockArea center tabs, left file tree, right backlinks/history, bottom terminal, status bar, actions, workspace.enc persistence, watcher → `SearchService::apply_changes` + tree refresh + tab reload, auto-sync every 5 min, OS drag and drop, flush on close, lock/switch vault, install `Platform` with `margin_fs::{reveal_in_file_manager, open_with_default_app}`).
4. Add new message keys used so far to both locales: `toast_vault_open_failed` (with `{error}`).
5. Updater on the existing `latest.json` + minisign key, `cargo packager` config, rewrite `release.yml` for the six targets.
6. End-to-end tests in `crates/apps/desktop/tests`, then measure against the budgets in docs/architecture.md; remove this file.

## Commands

- Core gate: see `mise.toml` tasks (`mise run gate`). Core-only clippy/test excluding UI: `--workspace --exclude margin-theme --exclude margin-i18n --exclude margin-workbench --exclude margin-editor --exclude margin-file-tree --exclude margin-spotlight --exclude margin-terminal --exclude margin-viewers --exclude margin-canvas --exclude margin-panels --exclude margin-onboarding --exclude margin-desktop`.
- Format: `cargo +nightly-2026-10-27 fmt --all`. Repo rules: `cargo xtask check`.

## UI contract

### Shared vocabulary (`margin-workbench`, owned by the integration owner — additive edits only, re-read before editing, tell `agent://Shell`)
- `WorkbenchEvent { Open(OpenRequest), Renamed { from, to }, Deleted(path), Saved(path), TagSelected(String), TreeChanged }` — every view that can open/rename/delete things implements `EventEmitter<WorkbenchEvent>`; the Workspace subscribes and routes.
- `OpenRequest { path: RelativePath, target: OpenTarget (ActivePane | NewTab | SplitRight), anchor: Option<String> }`.
- `TabContent: gpui_kit::component::dock::Panel + EventEmitter<WorkbenchEvent>` with `path()`, `rename(to, cx)`, `flush(cx)` (persist pending edits now), `reload(cx)` (file changed on disk). `TabHandle` is the object-safe wrapper the Workspace stores (`Entity<T>` implements it automatically).
- Actions (namespace `margin`): `OpenSpotlight, SearchVault, NewNote, ToggleSidebar, ToggleTerminal, ToggleViewMode, CloseTab, ReopenClosedTab, FindInNote, ReplaceInNote, PasteAsPlainText, OpenSettings, LockVault, SyncNow, ToggleBacklinks, ToggleHistory, OpenTrash`; key context `WORKSPACE_CONTEXT`. Editor-local actions live in margin-editor.
- `FileKind::of(path)` → Note | Canvas | Image | Pdf | Other.

### Public API each UI crate must expose (the Shell codes against exactly this)
- `margin-file-tree`: `FileTree` — `FileTree::new(window, cx) -> Self`; implements dock `Panel` (panel_name `"file-tree"`) + `EventEmitter<WorkbenchEvent>`; `reveal(&mut self, path: &RelativePath, window, cx)`, `refresh(&mut self, cx)` (after a watcher batch), `new_note(&mut self, window, cx)` (creates "Untitled.md" in the selected folder, emits Open + starts rename). Full original behaviour: virtualized tree, expand/collapse persisted (`expanded_folders()` / `set_expanded_folders(...)` for workspace state), sort by name/date, multi-select, inline rename (F2/triple-click), new file/folder, delete (to trash), duplicate, copy/cut/paste, move by drag and drop inside the tree, drop of OS files/folders (gpui `ExternalPaths`) = import, open in new tab/split, reveal in file manager via `margin_workbench::Platform::reveal(cx, &absolute_path)` (the Shell installs the OS implementation), attachment folder hidden, context menus, keyboard navigation.
- `margin-editor`: `NoteEditor` — `NoteEditor::new(path: RelativePath, window, cx) -> Self`; implements `TabContent`; `set_view_mode(ViewMode::{LivePreview, Raw}, cx)`, `toggle_view_mode(cx)`, `view_mode()`, `scroll_to_anchor(&str, window, cx)`, `cursor_offset()` / `set_cursor_offset(usize, window, cx)` (workspace state), handles `FindInNote`, `ReplaceInNote`, `PasteAsPlainText` when focused; saving through `FilesService::write_file` debounced, plus history snapshots via `History::save` with the original cadence; emits `Saved`, `Open` (links), `TagSelected`.
- `margin-viewers`: `ImageView`, `PdfView`, `UnknownFileView` — each `new(path, window, cx)` + `TabContent`. `UnknownFileView` and `PdfView` offer "open in default app" via `margin_workbench::Platform::open_external(cx, path_or_url)`; links in notes open the same way.
- `margin-canvas`: `CanvasView` — `new(path, window, cx)` + `TabContent`.
- `margin-spotlight`: `Spotlight` — `Spotlight::new(window, cx)`, `open(&mut self, SpotlightMode::{Files, FullText, Tag(String), Ask, Replace}, window, cx)`; rendered by the Shell as an overlay layer; emits `WorkbenchEvent::Open` and a `SpotlightEvent::Dismissed`.
- `margin-terminal`: `TerminalDock` — `new(window, cx)`; dock `Panel` (panel_name `"terminal"`), tabs of terminals, `new_terminal(window, cx)`.
- `margin-panels`: `BacklinksPanel` (panel_name `"backlinks"`) and `HistoryPanel` (`"history"`), each `new(window, cx)` + `set_note(Option<RelativePath>, cx)`; `open_trash_dialog(window, cx)`; `open_settings_dialog(window, cx)` (all original sections: vault, S3 sync incl. test connection/auto-sync/conflict strategy, AI incl. model list, appearance, language, export ZIP, export/import settings string, updates); `SyncStatus` view for the status bar (`new(window, cx)`, `sync_now(window, cx)`; emits `WorkbenchEvent::TreeChanged` after a sync).
- `margin-onboarding`: `Onboarding` — `new(window, cx)`; `EventEmitter<OnboardingEvent>` with `OnboardingEvent::Unlocked { name: String, root: PathBuf, vault: DerivedVault }`; all original screens: intro tour (skippable), vault picker (profiles from `AppServices.vaults`), wizard folder → name → passphrase (generate 12 words with `margin_crypto::generate_mnemonic`, or restore existing words), auto-login with the last session, remove vault, theme toggle. Folder picking via `cx.prompt_for_paths`.
