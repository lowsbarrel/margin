import prettier from 'eslint-config-prettier';
import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import { defineConfig } from 'eslint/config';
import globals from 'globals';
import ts from 'typescript-eslint';

export default defineConfig(
	{
		ignores: [
			'.svelte-kit/',
			'build/',
			'static/',
			'src/lib/paraglide/',
			'src/lib/bindings.ts',
			'src-tauri/target/',
			'src-tauri/gen/'
		]
	},
	js.configs.recommended,
	ts.configs.recommended,
	svelte.configs.recommended,
	prettier,
	svelte.configs.prettier,
	{
		languageOptions: { globals: { ...globals.browser, ...globals.node } },
		rules: {
			'no-undef': 'off',
			'no-empty': ['error', { allowEmptyCatch: true }]
		}
	},
	{
		files: ['**/*.svelte', '**/*.svelte.ts', '**/*.svelte.js'],
		languageOptions: {
			parserOptions: {
				projectService: true,
				extraFileExtensions: ['.svelte'],
				parser: ts.parser
			}
		}
	},
	{
		rules: {
			'@typescript-eslint/no-unused-vars': [
				'warn',
				{
					argsIgnorePattern: '^_',
					varsIgnorePattern: '^_',
					caughtErrorsIgnorePattern: '^_',
					destructuredArrayIgnorePattern: '^_'
				}
			],
			'svelte/no-at-html-tags': 'off',
			'svelte/prefer-svelte-reactivity': 'warn',
			'svelte/require-each-key': 'warn',
			'svelte/no-navigation-without-resolve': 'warn',
			'svelte/no-dom-manipulating': 'warn',
			'svelte/prefer-writable-derived': 'warn',
			'svelte/no-unused-svelte-ignore': 'warn',
			'@typescript-eslint/no-explicit-any': 'warn',
			'@typescript-eslint/no-unused-expressions': 'warn',
			'@typescript-eslint/no-empty-object-type': 'warn',
			'@typescript-eslint/no-this-alias': 'warn',
			'preserve-caught-error': 'warn',
			'no-useless-escape': 'warn',
			'no-useless-assignment': 'warn'
		}
	}
);
