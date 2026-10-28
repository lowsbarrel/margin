# Margin

A local-first Markdown editor. Notes are plain `.md` files on disk; sync blobs
are encrypted before they leave the machine. Pure Rust: a GPUI desktop app over
a hexagonal core. This file holds the engineering rules. The rest has one home
each: [docs/architecture.md](docs/architecture.md) (how it is built),
[docs/decisions/](docs/decisions/) (why, with evidence) and
[DESIGN.md](DESIGN.md) (how it looks, reads and behaves). Read them before
adding a file, and search for an existing home before creating one.

# The shape

Hexagonal. Dependencies point inward only, and `cargo xtask check-deps`
enforces it from every crate manifest.

| Layer        | Path                       | May depend on                                 |
| ------------ | -------------------------- | --------------------------------------------- |
| shared       | `crates/shared/<name>`     | shared, allowlisted pure crates               |
| features     | `crates/features/<name>`   | shared, features, allowlisted pure crates     |
| adapters     | `crates/adapters/<name>`   | shared, features, any external crate          |
| ui           | `crates/ui/<name>`         | shared, features, ui, `gpui-kit`              |
| apps         | `crates/apps/<name>`       | everything                                    |
| testing      | `crates/testing/<name>`    | shared, features, allowlisted pure crates; used only as a dev-dependency |
| tooling      | `xtask`                    | external crates only                          |

- **Features** are use cases. Fixed files, nothing else: `lib.rs` (docs, `mod`,
  `pub use`), `domain.rs` (entities and pure rules), `ports.rs` (traits the use
  cases need), `service.rs` (the use cases), `error.rs` (one `Error` enum).
- **Core owns formats, adapters move bytes.** Encryption, JSON shapes, the
  `.margin/` layout and S3 object keys are decided in core. An adapter
  implements one port and passes that port's conformance suite in
  `margin-test-support`, the same suite the in-memory fake passes.
- **Ports are object-safe.** Services hold `Arc<dyn Port>`; async methods return
  `margin_kernel::BoxFuture`. UI crates never see an adapter type.
- **Core does no I/O.** Nothing under `crates/shared` or `crates/features` names
  `std::fs`, `std::net`, `std::process`, `std::env` or `std::thread`, or depends
  on Tokio, GPUI, SQLite, HTTP or PTY crates.
- **The app is the composition root.** `crates/apps/desktop/src/container.rs`
  builds every adapter and service; views receive services, never construct
  them.

## One home per concept

| Concept                                           | Its one home                                     |
| ------------------------------------------------- | ------------------------------------------------ |
| Relative paths, timestamps, secrets, pagination (`Page`, `PageRequest`, `Cursor`), shared ports (`FileSystem`, `HostFiles`, `Clock`, `Timer`, `Watcher`) | `margin-kernel` |
| Retry with backoff for transient failures         | `margin-retry`                                   |
| Calendar dates, ISO 8601, day grouping            | `margin-dates`                                   |
| Key derivation, blob format, S3 object keys       | `margin-crypto`                                  |
| Markdown and Obsidian syntax parsing              | `margin-markdown`                                |
| A use case                                        | `crates/features/<feature>/src/service.rs`       |
| A port implementation                             | `crates/adapters/<adapter>/`                     |
| Fakes and port conformance suites                 | `margin-test-support`                                 |
| Colours, spacing, type scale                      | `margin-theme`                                   |
| User-facing copy                                  | `messages/{en,it}.json` via `margin-i18n`        |
| Wiring adapters into services                     | `crates/apps/desktop/src/container.rs`           |
| Dependency rules and repository invariants        | `xtask`                                          |
| Toolchain and dev tools                           | `rust-toolchain.toml`, `mise.toml`               |
| Lint configuration                                | `[workspace.lints]` in `Cargo.toml`, `clippy.toml` |
| Version                                           | `[workspace.package] version` in `Cargo.toml`    |
| License                                           | `LICENSE`                                        |

## Where new code goes

| You're adding                                   | Put it in                                         |
| ----------------------------------------------- | ------------------------------------------------- |
| A rule about notes, vaults, sync, history…      | `domain.rs` of that feature                       |
| Something a use case needs from the outside     | a trait in that feature's `ports.rs`              |
| Talking to disk, network, a database, a process | a new or existing crate under `crates/adapters/`  |
| A screen, panel or element                      | a crate under `crates/ui/`                        |
| A type two features share                       | `margin-kernel` (only if both really need it)     |
| A generic helper (retry, dates, paging, …)      | its existing shared crate; ask before adding a new one |
| A repository check                              | `xtask`                                           |

A file that grows past 400 lines, or a crate `src/` that wants a subdirectory,
means the crate does two things: split the crate.

# Invariants

- **Formats are frozen.** Vaults, the profile registry and buckets written by
  the Tauri releases (≤ 0.1.30) must open unchanged. docs/architecture.md lists the
  contract; compatibility fixtures in `margin-test-support` pin it.
- **Crypto lives in core** (`margin-crypto`) and is bit-exact with the Tauri
  releases. Only encrypted secrets touch disk. Never log or `Debug`-print a
  mnemonic, vault key, S3 secret or AI key: wrap them in types whose `Debug`
  redacts.
- The configured AI endpoint is the one place note text leaves the machine
  unencrypted, and only for a question the user asked.
- Hidden paths (leading `.`) are skipped by every walker, watcher, sync and
  export. Attachments live in one visible vault folder, hidden only in the tree.
- Heavy work never runs on the main thread: background executor for CPU, the
  Tokio runtime for network.
- No `unsafe`, anywhere, ever: `unsafe_code = "forbid"` in every crate and `cargo xtask
  check-invariants` rejects the keyword. No unstable features. No code before
  `main`.
- Colours and spacing come from `margin-theme`; no colour literal outside it.
- Every message key exists in every `messages/<locale>.json` with the same
  placeholders as `en.json`; no user-facing string is written in Rust code.
- One version: `[workspace.package] version`. The updater compares against it.
- The updater's public key lives in `crates/apps/desktop`; the private key
  exists only as a GitHub secret. Never commit a signing key, credential or
  mnemonic.
- Builds and passes on Windows, macOS and Linux, x64 and arm64.

# Style

Follow the [Google Rust Style Guide](https://google.github.io/styleguide/rust.html)
and, where it is silent, the official Rust style and API guidelines, with one
deliberate exception: **no comments** (below). Formatting and most rules are
enforced: `cargo +nightly fmt` (pinned in `mise.toml`) and `cargo clippy` with
the workspace lints, which deny warnings.

- **No comments.** None, anywhere: no `//`, no doc comments (`///`, `//!`), no
  block comments, no commented-out code, no TODOs; configuration files (TOML,
  YAML, JSON) carry none either. Names, types and small functions carry the
  meaning; a constraint the code cannot show goes into a name
  (`write_through_temp_file_for_atomicity`), an `expect` message or a test.
  An `#[expect(lint, reason = "…")]` reason is not a comment and is required
  for every lint exception. `cargo xtask check-invariants` enforces all of it.

- **Organization**: small crates; flat `src/` (no subdirectories, no `mod.rs`);
  name a file after its primary item; non-leaf modules (`lib.rs`) hold only
  `mod` declarations and `pub use`.
- **Imports**: no globs (except `use super::*` in tests); one module per `use`
  (`imports_granularity = "Module"`); import types and use them unqualified;
  call free functions through their module (`fs::read`, not `read`); import
  traits locally when used once.
- **Ownership**: borrow before you clone; accept `&str`, `&[T]`, `&Path`,
  `Option<&T>`. `Arc` only where shared ownership is real (services, entities).
- **APIs**: enums over unclear `bool` parameters; named fields over tuples;
  newtypes for values that mix up easily; `#[must_use]` where ignoring the
  result is a bug; implement std traits instead of look-alike methods; derive
  when possible; `Default` with `new()`.
- **Errors**: `Option` for obvious absence, `Result` with a typed error for
  recoverable failures, panics only for bugs. No `Result<_, String>`. `expect`
  with a reason over `unwrap`. Match instead of check-then-unwrap.
- **Concurrency**: never reach inside a poisoned lock; use `parking_lot`.
- **Panics**: a function that can panic says so in its name or its `expect`
  message, never in prose.
- **Iterators**: short combinator chains; a `for` loop when the chain branches or
  mutates; no chains in a `for` header; no `for_each`.
- **Naming**: Rust API naming (`as_`/`to_`/`into_`, `new`/`try_new`/`from_`,
  `foo`/`foo_mut`/`set_foo`/`take_foo`/`with_foo`); `~Ext` extension traits;
  descriptive lifetimes; no abbreviations a newcomer would not know.
- **Tests**: deterministic. No network, no wall clock, no shared temp paths.
  Test behaviour through the public API; never test wiring, forwarding or
  incidental defaults.

# Dependencies

- A third-party crate is used only when it fits Margin exactly: the API we
  need, nothing large we don't, permissive license, maintained, no native
  library to ship. If it doesn't fit perfectly, write the part we need from
  scratch in our own crate (a shared crate if it is pure, an adapter if it does
  I/O), with tests.
- Every version is declared once in `[workspace.dependencies]`; crates use
  `name.workspace = true`. Default features are off unless every one is used.
- Core crates may only use the allowlist in `xtask/src/dependency_rules.rs`;
  extending it is a reviewed decision, not a convenience.
- `cargo deny` gates licenses, advisories, bans and sources; `cargo shear`
  rejects unused dependencies.

# Naming

- Crates: `margin-<name>`; directories `crates/<layer>/<name>`; features are
  plural nouns (`vaults`, `terminals`) unless the noun is a mass noun (`history`,
  `search`, `sync`).
- Files: snake_case `.rs`.
- Message keys: snake_case.
- Branches: `<type>/<kebab-summary>`.
- Commits and PR titles: Conventional Commits, `type(scope): summary`,
  lowercase, ≤ 72 characters. Types: feat, fix, docs, refactor, perf, test,
  build, ci, chore, revert.

# Git & PRs

- Commit and push only when asked. Work on a branch off `main`; land it through
  a squash-merged pull request whose title becomes the commit subject.
- Hooks come from `lefthook.yml` (`mise run setup` installs them): pre-commit
  formats and lints, runs `check-invariants`, `check-deps` and secret scanning;
  commit-msg enforces Conventional Commits; pre-push runs the full gate.
- Releases are tags: `v<version>` builds, signs and publishes all six targets
  plus `latest.json` for the updater.
- Never force-push `main`. `Cargo.lock` is committed; CI builds with `--locked`.

# Workflow

`mise install` provides every tool (pinned in `mise.toml`); `rust-toolchain.toml`
pins Rust. Run tasks with `mise run <task>`:

| Task         | Runs                                                                 |
| ------------ | -------------------------------------------------------------------- |
| `setup`      | Install git hooks                                                    |
| `dev`        | `cargo run -p margin-desktop`                                        |
| `fmt`        | `cargo +nightly fmt`, `taplo fmt`                                    |
| `lint`       | format check, `cargo clippy --workspace --all-targets --all-features`, `typos`, `taplo check` |
| `check`      | `cargo xtask check` (deps, invariants, messages, secrets), `cargo deny check`, `cargo shear`, `jscpd` |
| `test`       | `cargo nextest run --workspace` and doc tests                        |
| `doc`        | `cargo doc --workspace --no-deps` with warnings denied               |
| `gate`       | everything above: what CI runs                                       |
| `bundle`     | build installers for the host target                                 |

Done means `mise run gate` passes, and a change with a visible surface was
exercised in the running app.
