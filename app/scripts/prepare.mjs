// Runs on every `bun install`. Two jobs: activate the enforced pre-commit hook,
// and generate the files the type checker needs (SvelteKit's .svelte-kit tree and
// the compiled Paraglide messages).
//
// A script rather than a shell one-liner because it must run identically on
// Windows, where `(a && b) || true` and `2>/dev/null` are not valid syntax.
// Nothing here may fail the install.
import { execSync } from 'node:child_process';

const run = (cmd) => execSync(cmd, { stdio: 'inherit' });

// Relative to the top of the working tree, which is where git runs hooks —
// so this resolves to <repo>/.githooks even though we are in app/.
try {
	run('git config core.hooksPath .githooks');
} catch {
	// Not a git checkout (tarball, vendored copy). Nothing to wire up.
}

try {
	run('bunx svelte-kit sync');
	run('bun run i18n:compile');
} catch {
	// A partially-installed tree can't sync yet; `bun run check` regenerates both.
}
