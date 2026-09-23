/** @type {import("prettier").Config} */
const config = {
	useTabs: true,
	singleQuote: true,
	trailingComma: 'none',
	printWidth: 100,
	// The Tailwind plugin sorts class lists and must be last.
	plugins: ['prettier-plugin-svelte', 'prettier-plugin-tailwindcss'],
	tailwindStylesheet: './src/app.css',
	overrides: [{ files: '*.svelte', options: { parser: 'svelte' } }]
};

export default config;
