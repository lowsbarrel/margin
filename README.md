<p align="center">
  <img src="media/logo.png" alt="Margin logo" width="128" />
</p>

# Margin

First of all, this name is banging. Like WOAH, marketing team (it's just me) deserves a raise.

Second of all, it's a note app. The most basic form of computer program known to man. Right up there with calculators and todo lists. And yet here we are, acting like it's revolutionary.

Imagine Obsidian but open source, with fewer features, and wrapped in a fancy Tauri wrapper so I can say **it's Rust based** at parties. Nobody at parties cares. I still say it.

I created this in 3 days using Claude because I refuse to learn Rust properly. Why spend weeks understanding lifetimes and borrow checkers when you can just throw money at the problem? Capitalism wins again.

It works surprisingly well. Like, *suspiciously* well. I'm scared to touch anything.

The UI is like 👌. Banging. I peaked here honestly.

If you want to contribute, do it. You're probably less lazy than I am. The bar is underground.

## What it does

- **Encrypted notes** — Every byte that leaves your machine is AES-256-GCM-SIV encrypted in Rust. Your sync target sees nothing but scrambled nonsense. Like my code
- **S3 sync** — Encrypted backup and sync to any S3-compatible bucket (Backblaze B2, Cloudflare R2, MinIO, etc.). 3-way merge with conflict resolution because I'm overengineering a note app
- **12-word passphrase** — No accounts, no emails, no passwords. A BIP-39 mnemonic *is* your identity and encryption key. Lose it and your notes are gone forever. No I will not help you recover them. I literally can't
- **Plain Markdown on disk** — Notes are stored as `.md` files in a directory you choose. Readable by any tool. You can leave me whenever you want. I won't hold your files hostage like some apps I could name
- **Rich editor** — WYSIWYG Tiptap editor with tables, math, wiki-links, callouts, code blocks, slash commands, and more. It's actually kinda fire
- **Graph view** — Visualise connections between notes via `[[wiki-links]]`. Makes you feel like a genius detective with a conspiracy board
- **File history** — Timestamped snapshots of every file, stored locally. Because you WILL accidentally delete that paragraph you spent 40 minutes writing
- **Multi-vault** — Multiple named vaults, each with its own mnemonic and folder. For when you need to separate your work notes from your unhinged personal thoughts
- **Desktop-only** — Native Tauri 2 app. All crypto runs in Rust, never in JavaScript. Because letting JS handle encryption is like letting a toddler hold a sword

## Features

### Editor
- Tables with resizable columns
- Task lists (nested, for when your procrastination has subcategories)
- Code blocks with syntax highlighting
- Math — inline `$…$` and block `$$$…$$$` via KaTeX (I don't use this but it makes me look smart)
- Wiki-links `[[page]]` with click-to-navigate
- Callouts — `info`, `note`, `success`, `warning`, `danger` (the five moods of documentation)
- File embeds and image attachments
- Slash command menu (`/`) — the single most satisfying UX pattern ever invented
- `@` mention menu to insert wiki-links inline — because typing `[[` with your bare hands is beneath you
- Bubble toolbar for formatting
- Find & replace
- Highlight, underline, superscript, subscript, text colour, text alignment
- Block drag handles — grab and rearrange paragraphs like furniture
- Smart typography (curly quotes, em dashes, etc.)
- Clickable links
- Image lightbox — click any image to enlarge it. Squinting is optional
- Character count (for when you're writing an essay and need exactly 500 words)
- Export current note to PDF

### Vault & files
- File tree with folders, right-click context menu, drag-and-drop
- Quick switcher for instant note navigation (Ctrl+P gang rise up)
- Full-text search across the entire vault — find that one sentence you wrote at 3am
- Tag system with sidebar filtering
- Favourites — pin files to the top like they're VIP
- Multi-pane split view with tabs — for when one note isn't enough to contain your thoughts
- Canvas editor for free-form visual notes
- Built-in image viewer and PDF viewer (because alt-tabbing is for quitters)
- Native file drag to desktop
- Copy/cut/paste files in the tree like a civilised person
- Zip export of the whole vault

### Sync (S3)
- Encrypted sync to any S3-compatible bucket — Backblaze B2, Cloudflare R2, MinIO, etc.
- 3-way merge: compares base, local and remote states — only uploads/downloads what changed. It's like git but for people who don't want to learn git
- Conflict strategy: local wins; conflicting remote version saved as `file.sync-conflict-<timestamp>.md`. Your version is always right. I believe in you
- All file types supported (`.md`, `.png`, `.pdf`, `.canvas`, …)
- Auto-sync every 5 minutes (optional) or manual via the status bar
- Hidden `.margin/` folder excluded automatically

### Security
- All crypto in Rust — JavaScript never touches keys or plaintext. JS has done enough damage
- AES-256-GCM-SIV authenticated encryption, 96-bit random nonces, nonce-misuse resistant. I don't fully understand what half of that means but it sounds very secure
- BIP-39 key derivation: 128-bit entropy → 512-bit seed (PBKDF2-HMAC-SHA512) → SHA-256 split into vault ID + encryption key. Yeah I definitely typed all that myself
- Session encrypted at rest with a per-device random key (`device.key`, permissions `0600`)
- Settings stored encrypted per vault (`settings.enc`), exportable as a portable encrypted string
- File history stored in `.margin/history/` inside the vault — never leaves your machine
- CORS restricted to `tauri://localhost` by default
- No telemetry, no analytics, no phone-home. I don't want your data. I have enough problems

### App
- Custom theme editor — create, edit, import/export themes. Make it ugly. I dare you
- OTA auto-updater — updates itself so you don't have to re-download like it's 2005
- Toast notifications — little popups that tell you things went well (or didn't)

## Architecture

| Layer | Technology |
| ----- | ---------- |
| Client | Tauri 2 (Rust) + SvelteKit + TypeScript |
| Editor | Tiptap + ProseMirror |
| Encryption | Rust — AES-256-GCM-SIV (`aes-gcm-siv` crate) |
| Key derivation | BIP-39 mnemonic → seed → SHA-256 split (vault ID + encryption key) |
| S3 sync | Rust (`aws-sdk-s3` / `rust-s3`) — encrypted 3-way merge |
| i18n | Paraglide — English, Italian (because I'm Italian and I do what I want) |

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
│       └── src/                # Rust commands (written by Claude, let's be honest)
│           ├── crypto.rs       # AES-256-GCM-SIV encrypt/decrypt
│           ├── fs.rs           # Filesystem operations
│           ├── history.rs      # File snapshot management
│           ├── s3.rs           # S3 client & operations
│           ├── session.rs      # Multi-vault profiles, device key
│           └── settings.rs     # Encrypted settings load/save/export
```

## Prerequisites

- Node.js 20+, [pnpm](https://pnpm.io/), Rust toolchain, [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/)
- A functioning computer (shocking, I know)

## Getting started

```bash
cd app
pnpm install
pnpm tauri dev
```

If that works on the first try, buy a lottery ticket.

To build a release binary:

```bash
pnpm tauri build
```

## How it works

1. **Generate or enter a mnemonic** — A 12-word BIP-39 passphrase is your identity. Write it down. Tattoo it. I don't care. Lose it and the vault is unrecoverable. I'm not even being dramatic, it's literally math
2. **Pick a folder** — Choose any local directory; your `.md` files live there. It's your folder. I'm not the boss of you
3. **Unlock** — The mnemonic derives a vault ID and an AES-256-GCM-SIV encryption key. Both are derived client-side and never transmitted anywhere. Not even I can see your stuff
4. **Write** — WYSIWYG Tiptap editor; use `/` for the slash menu or type Markdown syntax directly. Go wild
5. **Sync** — Configure S3 credentials in Settings. Hit sync in the status bar or let auto-sync run every 5 minutes. Files are encrypted in Rust before upload; the bucket holds only ciphertext. Your cloud provider gets to store your secrets without knowing a single one. Beautiful
6. **History** — Snapshots are saved automatically to `.margin/history/<file>/`. Open the history panel to browse and restore any version. Past you is looking out for future you

## License

[AGPL-3.0](LICENSE) — Because corporate vultures can contribute back or get lost
