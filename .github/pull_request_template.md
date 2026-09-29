## What & why

## Checklist

- [ ] Title is Conventional Commits (`type(scope): summary`, ≤72 chars); it becomes the squash subject
- [ ] `bun run lint && bun run check && bun run check:invariants && bun run check:secrets` pass in `app/`
- [ ] `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` pass in `app/src-tauri`
- [ ] Paraglide messages (`app/messages/`) updated if copy changed; bindings regenerated (`bun run gen:bindings`) if a Rust command signature changed
- [ ] Verified the change in a local `bun run tauri dev` run
