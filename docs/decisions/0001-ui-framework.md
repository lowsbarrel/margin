# 0001 · UI framework: GPUI through GPUI Kit

- Status: accepted
- Date: 2026-10-28

## Context

Margin moves from Tauri (SvelteKit in the system webview) to pure Rust with a
native GUI. The goals, in order: the fastest program, the least memory and CPU,
the lightest installer, on Windows, macOS and Linux (x64 and arm64). Candidates:
GPUI, iced, egui and Slint.

## Decision

The UI is [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui)
(Zed's GPU UI framework) consumed through
[`gpui-kit`](https://github.com/longbridge/gpui-kit), which pins a published
GPUI snapshot and adds a component library (dock with split panes and tabs,
virtual lists, tree, menus, dialogs, notifications, inputs, a rope-backed code
editor).

## Measurements

Each candidate rendered the same window: a heading and its stock multi-line
editor holding a 2,000-line Markdown document. Release build, `lto = true`,
`codegen-units = 1`, `strip = true`, `panic = "abort"`, Windows 11 x64; memory
read 4 s after the window appeared, idle CPU over the next 5 s, startup is the
second (warm) launch.

| Framework / renderer        | exe     | zstd -19 | Startup | Working set | Private | Idle CPU / 5 s |
| --------------------------- | ------- | -------- | ------- | ----------- | ------- | -------------- |
| egui 0.37, wgpu (default)   | 9.9 MB  | 3.5 MB   | 43 ms   | 318 MB      | 280 MB  | 0 ms           |
| egui 0.37, glow, no sysfont | 4.9 MB  | 1.9 MB   | 40 ms   | 86 MB       | 47 MB   | 16 ms          |
| iced 0.14, wgpu (default)   | 7.2 MB  | 2.5 MB   | 64 ms   | 172 MB      | 138 MB  | 16 ms          |
| iced 0.14, tiny-skia (CPU)  | 3.0 MB  | 1.1 MB   | 48 ms   | 55 MB       | 46 MB   | 0 ms           |
| Slint 1.18, femtovg         | 9.7 MB  | 4.4 MB   | 48 ms   | 115 MB      | 74 MB   | 0 ms           |
| Slint 1.18, software (CPU)  | 9.2 MB  | 4.2 MB   | 47 ms   | 47 MB       | 34 MB   | 0 ms           |
| GPUI (gpui-kit 0.8.1), DX11 | 13.8 MB | 4.2 MB   | 192 ms  | 68 MB       | 98 MB   | 94 ms (caret)  |

For reference, the Tauri 0.1.30 Windows installer is 12.7 MB and its Linux
AppImage 89.8 MB, before WebView2 or WebKitGTK memory is counted.

## Why GPUI

- **The editor decides.** Margin's core is an Obsidian-style live-preview
  editor: mixed font sizes per line, hidden markup that reveals at the caret,
  inline widgets (math, diagrams, images, tables), IME, multiple cursors. That
  needs shaped text runs with per-run font, colour and background, index↔x hit
  testing, a custom element with its own layout and paint, and a platform input
  handler. GPUI exposes all of it (`shape_line`/`shape_text`, `Element`,
  `EntityInputHandler`) and Zed proves it at scale. Slint exposes none of it to
  Rust; egui and iced expose part of it.
- **Lowest memory among GPU renderers** (68 MB working set, against 86–318 MB),
  with native text (DirectWrite, Core Text) instead of a bundled rasterizer.
- **CPU renderers lose on speed.** iced/tiny-skia and Slint/software are
  lighter at rest but rasterize every frame on the CPU; scrolling a long note on
  a high-DPI, high-refresh display costs more than the memory they save.
- **The component library is application-grade**: dock layout, virtual lists,
  tree and dialogs are hundreds of lines Margin does not write.
- **The installer cost is small.** GPUI compresses to about the same size as
  Slint; the 3 MB gap to iced is dwarfed by SQLite, rustls and the PDF renderer
  every candidate would ship.

Costs we accept: GPUI is pre-1.0 (we pin `gpui-kit` exactly and upgrade on
purpose), builds are slow, and the first launch compiles shaders (~1.5 s).
