<p align="center">
  <img alt="Margin logo" src="media/logo.png" width="128" />
</p>

<div align="center">
    <a href="https://github.com/lowsbarrel/margin/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/lowsbarrel/margin?style=flat-square" /></a>
    <a href="https://github.com/lowsbarrel/margin/actions/workflows/ci.yml"><img alt="CI status" src="https://img.shields.io/github/actions/workflow/status/lowsbarrel/margin/ci.yml?style=flat-square&branch=main" /></a>
    <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/license-MIT-blue?style=flat-square" /></a>
</div>

Margin is a local-first Markdown editor.

- Your notes are plain `.md` files in a folder you choose.
- It is written in Rust and Svelte, on Tauri.
- You find anything with `Cmd+P`, and ask your notes questions with `?` using your own AI provider.
- You can switch any note to raw Markdown and open a real terminal in your vault.
- You can sync to any S3 bucket, end-to-end encrypted with a key from a 12-word passphrase.

History and trash keep mistakes recoverable. No account, no telemetry.

[Download](https://github.com/lowsbarrel/margin/releases/latest) | [Changelog](https://github.com/lowsbarrel/margin/releases)

## Development

Development requires Bun 1.3 or later, a Rust toolchain and the
[Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform.

See [AGENTS.md](AGENTS.md) for engineering, tooling, and verification conventions.

```bash
cd app
bun install
bun run tauri dev
```

Run the checks from `app/` before opening a pull request:

```bash
bun run lint
bun run check
bun run check:invariants
cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test
```

## Contributing

Margin is open for contributions.

## License

Margin is licensed under the [MIT License](LICENSE).
