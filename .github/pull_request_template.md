## What & why

<!-- What changed and the reason. Link any issue. -->

## Checklist

- [ ] Title is Conventional Commits (`type(scope): summary`, ≤72 chars) — it becomes the squash subject
- [ ] `bun run lint && bun run check && bun run check:invariants && bun run check:secrets` pass, and `cargo fmt --check && cargo clippy -- -D warnings` in `app/src-tauri`
- [ ] Paraglide messages (`app/messages/`) updated if copy changed; bindings regenerated (`bun run gen:bindings`) if a Rust command signature changed
- [ ] Verified the change in a local `bun run tauri dev` run
