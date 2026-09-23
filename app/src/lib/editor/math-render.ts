import type katex from 'katex';

type Katex = typeof katex;

let katexModule: Katex | null = null;
let katexLoad: Promise<void> | null = null;

function loadKatex(): Promise<void> {
	katexLoad ??= import('katex').then(
		(mod) => {
			katexModule = mod.default;
		},
		(err) => {
			console.error('KaTeX failed to load:', err);
		}
	);
	return katexLoad;
}

const katexCache = new Map<string, string>();
const KATEX_CACHE_MAX = 512;

export function cachedKatex(text: string, displayMode: boolean): string | null {
	const key = `${displayMode ? 'D' : 'I'}:${text}`;
	const cached = katexCache.get(key);
	if (cached !== undefined) {
		katexCache.delete(key);
		katexCache.set(key, cached);
		return cached;
	}
	if (!katexModule) {
		void loadKatex();
		return null;
	}
	const html = katexModule.renderToString(text, { displayMode, throwOnError: false });
	if (katexCache.size >= KATEX_CACHE_MAX) {
		const first = katexCache.keys().next().value;
		if (first !== undefined) katexCache.delete(first);
	}
	katexCache.set(key, html);
	return html;
}

// A failed load resolves too, so repaint only once `katexModule` is set or the retry spins forever.
export function renderWhenReady(paint: () => void): void {
	if (katexModule) return;
	void loadKatex().then(() => {
		if (katexModule) paint();
	});
}

export function escapeHtml(str: string): string {
	return str
		.replace(/&/g, '&amp;')
		.replace(/</g, '&lt;')
		.replace(/>/g, '&gt;')
		.replace(/"/g, '&quot;');
}
