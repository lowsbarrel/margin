// Type-aware deprecation gate (`pnpm check:deprecated`). Flags any use of an
// @deprecated symbol — renamed Lucide icons, deprecated Tiptap/SvelteKit/
// Tauri-plugin APIs, your own @deprecated tags. Needs full type info, so it's
// slow and runs as its own CI job — never in `pnpm lint` or the pre-commit hook.
import base from './eslint.config.js';

export default [
	...base,
	{
		files: ['**/*.ts', '**/*.svelte', '**/*.svelte.ts'],
		languageOptions: { parserOptions: { projectService: true } },
		rules: { '@typescript-eslint/no-deprecated': 'error' }
	}
];
