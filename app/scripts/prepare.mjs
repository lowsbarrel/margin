import { execSync } from 'node:child_process';

const run = (cmd) => execSync(cmd, { stdio: 'inherit' });

// Git resolves core.hooksPath against the working-tree root, not this package.
try {
	run('git config core.hooksPath .githooks');
} catch {}

// A hook or a generated tree is a convenience: nothing here may fail the install.
try {
	run('bunx svelte-kit sync');
	run('bun run i18n:compile');
} catch {}
