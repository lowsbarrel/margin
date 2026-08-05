<div align="center">

<img src="media/logo.png" alt="Margin logo" width="128" />

# Margin

**A local-first Markdown note app. Plain `.md` files on your disk, end-to-end encrypted sync to any S3 bucket.**

Notes stay as ordinary Markdown in a folder you choose. Everything that leaves the machine is encrypted in Rust first, under a key derived from a 12-word passphrase that no server ever sees. No account, no email, no telemetry.

[![CI](https://github.com/lowsbarrel/margin/actions/workflows/ci.yml/badge.svg)](https://github.com/lowsbarrel/margin/actions/workflows/ci.yml)
![Tauri 2](https://img.shields.io/badge/Tauri_2-24C8DB?logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)
![Svelte 5](https://img.shields.io/badge/Svelte_5-FF3E00?logo=svelte&logoColor=white)
![SvelteKit 2](https://img.shields.io/badge/SvelteKit_2-FF3E00?logo=svelte&logoColor=white)
![TypeScript](https://img.shields.io/badge/TypeScript-3178C6?logo=typescript&logoColor=white)
![Tiptap](https://img.shields.io/badge/Tiptap-000000?logo=tiptap&logoColor=white)
![AES-256-GCM-SIV](https://img.shields.io/badge/AES--256--GCM--SIV-4B32C3)
[![License](https://img.shields.io/badge/License-see_LICENSE-blue)](LICENSE)

[Get started](#getting-started) · [How it works](#how-it-works) · [Features](#features) · [Security](#security) · [Stack](#stack)

</div>

---

## What this is

A **desktop note app** for people who want their notes to outlive the app. Every
note is a `.md` file in a directory you pick: readable by any editor, greppable
by any tool, portable the day you decide to leave.

The other half is sync, and that's where the constraint lives: the vault can be
mirrored to any S3-compatible bucket, but the bucket only ever holds ciphertext.
Encryption happens in Rust before upload; the key is derived on your machine from
a BIP-39 mnemonic and is never transmitted. Your storage provider stores your
notes without being able to read a single one.

Imagine Obsidian but open source, with fewer features, wrapped in Tauri so I can
say **it's Rust based** at parties. Nobody at parties cares. I still say it.

## Features

| Area       | What you get                                                                                                                         |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------- -|
| Storage    | Plain Markdown `.md` files in a folder you choose. No proprietary format, no lock-in                                                 |
| Editor     | Tiptap WYSIWYG: tables, task lists, code blocks, KaTeX math, callouts, wiki-links, slash menu, `@` mentions, find & replace          |
| Encryption | AES-256-GCM-SIV in Rust with 96-bit random nonces. JavaScript never touches keys or plaintext                                        |
| Identity   | A 12-word BIP-39 mnemonic derives the vault ID and the encryption key. No accounts, no email, no password reset                      |
| Sync       | Encrypted 3-way merge to any S3-compatible bucket (Backblaze B2, Cloudflare R2, MinIO); manual or every 5 minutes                    |
| Navigation | Quick switcher, full-text search across the vault, tags with sidebar filtering, favourites, graph view of `[[wiki-links]]`           |
| Workspace  | Multi-pane split view with tabs, canvas editor, built-in image and PDF viewers, drag files out to the desktop, zip export            |
| History    | Timestamped snapshots of every file in `.margin/history/`: browsable, restorable, never uploaded anywhere                            |
| Vaults     | Multiple named vaults, each with its own mnemonic and its own folder                                                                 |
| Appearance | Dark and light themes, Inter + JetBrains Mono, image lightbox, toast notifications                                                   |
| Privacy    | No telemetry, no analytics, no phone-home. CORS locked to `tauri://localhost`                                                        |
| Updates    | OTA auto-updater; macOS, Linux and Windows binaries built and signed by CI on every tag                                              |

<details>
<summary>The full editor list</summary>

Tables with resizable columns · nested task lists · syntax-highlighted code
blocks · inline `$…$` and block `$$$…$$$` math via KaTeX · `[[wiki-links]]` with
click-to-navigate · `info` / `note` / `success` / `warning` / `danger` callouts ·
file embeds and image attachments · slash command menu · `@` mention menu for
inline wiki-links · bubble formatting toolbar · find & replace · highlight,
underline, superscript, subscript, text colour, alignment · block drag handles ·
smart typography · clickable links · image lightbox · character count · export
the current note to PDF.

</details>

## Getting started

```bash
cd app
pnpm install
pnpm tauri dev
```

If that works on the first try, buy a lottery ticket.

```bash
pnpm tauri build   # release binary for the current platform
```

## How it works

1. **Generate or enter a mnemonic.** A 12-word BIP-39 passphrase is your
   identity. Write it down. Lose it and the vault is unrecoverable: not
   "contact support" unrecoverable, *math* unrecoverable.
2. **Pick a folder.** Any local directory; your `.md` files live there.
3. **Unlock.** The mnemonic derives a vault ID and an AES-256-GCM-SIV key, both
   client-side, neither transmitted.
4. **Write.** WYSIWYG editing, or type Markdown syntax directly and let it
   convert. `/` opens the slash menu.
5. **Sync.** Add S3 credentials in Settings, then sync from the status bar or let
   auto-sync run. Files are encrypted in Rust before upload; conflicts resolve
   local-wins, with the remote version kept as
   `file.sync-conflict-<timestamp>.md`.
6. **History.** Snapshots are written automatically to `.margin/history/<file>/`.
   The history panel browses and restores them.

## Security

| Concern         | How it's handled                                                                                               |
| --------------- | ------------------------------------------------------------------------------------------------------------ --|
| Cipher          | AES-256-GCM-SIV, authenticated, nonce-misuse resistant, 96-bit random nonces                                   |
| Key derivation  | BIP-39: 128-bit entropy → 512-bit seed (PBKDF2-HMAC-SHA512) → SHA-256, split into vault ID + encryption key    |
| Crypto boundary | All encryption lives in Rust behind Tauri commands; the frontend passes paths and receives plaintext, no keys  |
| Session at rest | Encrypted with a per-device random key (`device.key`, mode `0600`)                                             |
| Settings        | Stored encrypted per vault (`settings.enc`), exportable as a portable encrypted string                         |
| What sync sees  | Ciphertext only. The `.margin/` folder, history included, never leaves the machine                             |
| Network         | CORS restricted to `tauri://localhost`; no analytics, no crash reporting, no phone-home                        |

## Stack

| Layer          | Choice                                                                      |
| -------------- | -------------------------------------------------------------------------- -|
| Shell          | Tauri 2: Rust core, system webview, no bundled browser                      |
| Frontend       | SvelteKit 2 with Svelte 5 runes + TypeScript, static-adapter prerendered    |
| Editor         | Tiptap 3 on ProseMirror, with custom extensions per feature                 |
| Encryption     | Rust `aes-gcm-siv`, providing AES-256-GCM-SIV                               |
| Key derivation | BIP-39 mnemonic → seed → SHA-256 split (vault ID + encryption key)          |
| Sync           | Rust S3 client, encrypted 3-way merge against base / local / remote state   |
| Rendering      | KaTeX for math, lowlight for code, Mermaid for diagrams, pdf.js for PDFs    |
| i18n           | Paraglide: English and Italian (because I'm Italian and I do what I want)   |
| CI             | GitHub Actions: `svelte-check` + `cargo check` on PRs, tagged releases      |

## Project structure

```text
margin/
├── app/                        # Tauri + SvelteKit desktop client
│   ├── src/
│   │   ├── lib/
│   │   │   ├── components/     # Editor, Sidebar, FileTree, Login, StatusBar, GraphView, …
│   │   │   ├── editor/         # Tiptap extensions (math, wiki-link, callout, slash-menu, …)
│   │   │   ├── crypto/         # TypeScript bridge to Rust crypto commands
│   │   │   ├── fs/             # TypeScript bridge to Rust filesystem commands
│   │   │   ├── history/        # File snapshot bridge
│   │   │   ├── s3/             # S3 sync engine
│   │   │   ├── settings/       # Encrypted settings bridge
│   │   │   ├── stores/         # Svelte 5 runes stores (vault, files, editor, graph, …)
│   │   │   └── sync/           # S3 sync orchestration
│   │   └── routes/             # SvelteKit pages
│   └── src-tauri/
│       └── src/                # Rust commands
│           ├── crypto.rs       # AES-256-GCM-SIV encrypt/decrypt
│           ├── fs.rs           # Filesystem operations
│           ├── history.rs      # File snapshot management
│           ├── s3.rs           # S3 client & operations
│           ├── session.rs      # Multi-vault profiles, device key
│           └── settings.rs     # Encrypted settings load/save/export
```

**The one rule:** plaintext and keys stay on the Rust side. If a feature needs to
touch either from JavaScript, it gets a Tauri command instead.

## Contributing

Do it. You're probably less lazy than I am, and the bar is underground.

## Requirements

Node ≥ 24 · pnpm · a Rust toolchain · the
[Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your
platform.

## License

See [LICENSE](LICENSE). Because corporate vultures can contribute back or get
lost.
