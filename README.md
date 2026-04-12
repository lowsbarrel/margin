<p align="center">
  <img src="media/logo.png" alt="Margin logo" width="128" />
</p>

# Margin

A client-side encrypted, S3-synced Markdown editor. Your notes. Your keys.

## What it does

- **Encrypted notes** — Every byte that leaves your machine is AES-256-GCM-SIV encrypted in Rust. The sync target only ever sees ciphertext
- **S3 sync** — Encrypted backup and sync to any S3-compatible bucket (Backblaze B2, Cloudflare R2, MinIO, etc.). 3-way merge with conflict resolution
- **12-word passphrase** — No accounts, no emails, no passwords. A BIP-39 mnemonic *is* your identity and encryption key
- **Plain Markdown on disk** — Notes are stored as `.md` files in a directory you choose. Readable by any tool
- **Rich editor** — WYSIWYG Tiptap editor with tables, math, wiki-links, callouts, code blocks, slash commands, and more
- **Graph view** — Visualise connections between notes via `[[wiki-links]]`
- **File history** — Timestamped snapshots of every file, stored locally inside the vault
- **Multi-vault** — Multiple named vaults, each with its own mnemonic and folder. Session encrypted with a per-device key
- **Desktop-only** — Native Tauri 2 app. All crypto runs in Rust, never in JavaScript

## Features

### Editor
- Tables with resizable columns
- Task lists (nested)
- Code blocks with syntax highlighting
- Math — inline `$…$` and block `$$$…$$$` via KaTeX
- Wiki-links `[[page]]` with click-to-navigate
- Callouts — `info`, `note`, `success`, `warning`, `danger`
- File embeds and image attachments
- Slash command menu (`/`)
- Bubble toolbar for formatting
- Find & replace
- Highlight, underline, superscript, subscript, text colour, text alignment
- Character count
- Export current note to PDF

### Vault & files
- File tree with folders, right-click context menu, drag-and-drop
- Quick switcher for instant note navigation
- Favourites — pin files to the top
- Canvas editor for free-form visual notes
- Built-in image viewer and PDF viewer

### Sync (S3)
- Encrypted sync to any S3-compatible bucket — Backblaze B2, Cloudflare R2, MinIO, etc.
- 3-way merge: compares base, local and remote states — only uploads/downloads what changed
- Conflict strategy: local wins; conflicting remote version saved as `file.sync-conflict-<timestamp>.md`
- All file types supported (`.md`, `.png`, `.pdf`, `.canvas`, …)
- Auto-sync every 5 minutes (optional) or manual via the status bar
- Hidden `.margin/` folder excluded automatically

### WebSocket relay (optional)
- Self-hosted Python/FastAPI server with Redis for real-time collaboration
- Zero-knowledge relay — ciphertext only, Redis runs without disk persistence

### Security
- All crypto in Rust — JavaScript never touches keys or plaintext
- AES-256-GCM-SIV authenticated encryption, 96-bit random nonces, nonce-misuse resistant
- BIP-39 key derivation: 128-bit entropy → 512-bit seed (PBKDF2-HMAC-SHA512) → SHA-256 split into vault ID + encryption key
- Session encrypted at rest with a per-device random key (`device.key`, permissions `0600`)
- Settings stored encrypted per vault (`settings.enc`), exportable as a portable encrypted string
- File history stored in `.margin/history/` inside the vault — never leaves your machine
- CORS restricted to `tauri://localhost` by default
- No telemetry, no analytics, no phone-home

## Architecture

| Layer | Technology |
| ----- | ---------- |
| Client | Tauri 2 (Rust) + SvelteKit + TypeScript |
| Editor | Tiptap + ProseMirror |
| Encryption | Rust — AES-256-GCM-SIV (`aes-gcm-siv` crate) |
| Key derivation | BIP-39 mnemonic → seed → SHA-256 split (vault ID + encryption key) |
| S3 sync | Rust (`aws-sdk-s3` / `rust-s3`) — encrypted 3-way merge |
| WebSocket relay | Python — FastAPI, Redis (RAM-only) |
| i18n | Paraglide — English, Italian |

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
├── server/                     # Python FastAPI WebSocket relay (optional)
│   ├── main.py
│   └── src/
│       ├── config.py           # MARGIN_* env vars
│       ├── models.py           # Pydantic message models
│       ├── routers/            # health, ws
│       └── services/           # redis_store, ws fan-out & bootstrap
└── docs/
    └── PROJECT_IDEA.md
```

## Prerequisites

- **Client**: Node.js 20+, [pnpm](https://pnpm.io/), Rust toolchain, [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/)
- **Server** (optional): Python 3.13+, [uv](https://docs.astral.sh/uv/), Redis 7+

## Getting started

### Client

```bash
cd app
pnpm install
pnpm tauri dev
```

To build a release binary:

```bash
pnpm tauri build
```

### WebSocket relay server (optional)

```bash
cd server
uv sync
uv run python main.py
```

The server starts on `http://0.0.0.0:8000` by default. Configuration via `MARGIN_*` environment variables:

| Variable | Default | Description |
| -------- | ------- | ----------- |
| `MARGIN_REDIS_URL` | `redis://localhost:6379/0` | Redis connection URL |
| `MARGIN_HOST` | `0.0.0.0` | Bind address |
| `MARGIN_PORT` | `8000` | Port |
| `MARGIN_VAULT_EVICTION_DAYS` | `180` | Days before idle vault ciphertext expires |
| `MARGIN_WS_MAX_MESSAGE_BYTES` | `4194304` | Max WebSocket message size (4 MB) |
| `MARGIN_CORS_ORIGINS` | `["tauri://localhost"]` | Allowed CORS origins |

```bash
# Or with Docker
cd server
docker compose up --build
```

## How it works

1. **Generate or enter a mnemonic** — A 12-word BIP-39 passphrase is your identity. Write it down; lose it and the vault is unrecoverable
2. **Pick a folder** — Choose any local directory; your `.md` files live there
3. **Unlock** — The mnemonic derives a vault ID and an AES-256-GCM-SIV encryption key. Both are derived client-side and never transmitted anywhere
4. **Write** — WYSIWYG Tiptap editor; use `/` for the slash menu or type Markdown syntax directly
5. **Sync** — Configure S3 credentials in Settings. Hit sync in the status bar or let auto-sync run every 5 minutes. Files are encrypted in Rust before upload; the bucket holds only ciphertext
6. **History** — Snapshots are saved automatically to `.margin/history/<file>/`. Open the history panel to browse and restore any version

## License

[AGPL-3.0](LICENSE)
