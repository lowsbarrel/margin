declare module 'turndown-plugin-gfm' {
	type Plugin = (service: { addRule(key: string, rule: unknown): void }) => void;
	export const gfm: Plugin;
	export const tables: Plugin;
	export const strikethrough: Plugin;
	export const taskListItems: Plugin;
}
