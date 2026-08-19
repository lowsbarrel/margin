## What & why

<!-- What changed and the reason. Link any issue. -->

## Checklist

- [ ] Title is Conventional Commits (`type(scope): summary`, ≤72 chars) — it becomes the squash subject
- [ ] `pnpm lint && pnpm check && pnpm check:invariants && pnpm check:secrets` pass, and `cargo fmt --check && cargo clippy -- -D warnings` in `app/src-tauri`
- [ ] Paraglide messages (`app/messages/`) updated if copy changed; bindings regenerated (`pnpm gen:bindings`) if a Rust command signature changed
- [ ] Verified the change in a local `pnpm tauri dev` run
