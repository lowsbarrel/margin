# 0002 · Native replacements for web-only libraries

- Status: accepted
- Date: 2026-10-28

## Context

The Tauri app leaned on browser libraries with no drop-in Rust equivalent. Each
replacement had to stay pure Rust (no embedded JavaScript engine or webview) and
keep the installer small.

## Decision

| Tauri app                    | Native rewrite                                                   |
| ---------------------------- | ---------------------------------------------------------------- |
| CodeMirror 6 live preview    | Own GPUI element over a rope; parsing in `margin-markdown`       |
| CodeMirror raw mode          | Same element with decorations off                                |
| KaTeX                        | [RaTeX](https://github.com/erweixin/RaTeX) → SVG → GPUI, as Zed  |
| Mermaid                      | `mermaid-rs-renderer` → SVG → GPUI                               |
| PDF.js                       | `hayro` (pure Rust rasterizer and interpreter)                   |
| xterm.js                     | `alacritty_terminal` grid painted by a GPUI element              |
| turndown (HTML paste)        | `htmd`                                                           |
| markdown-it + jsPDF export   | HTML export (print to PDF from a browser)                        |
| lowlight                     | `gpui-kit` highlighter (tree-sitter)                             |
| Tauri dialog/opener/updater  | `rfd`, OS open/reveal, own updater on the same `latest.json`     |
| Paraglide                    | `messages/{en,it}.json` loaded by `margin-i18n`, same keys       |

Math follows what Zed does for GPUI (RaTeX, KaTeX-compatible layout rendered as
SVG). Exporting a note to PDF is replaced by a standalone HTML export the user
can print from a browser.

Each listed crate is adopted only if it fits exactly (AGENTS.md, Dependencies); where it does not, Margin implements the part it needs in its own crate.
