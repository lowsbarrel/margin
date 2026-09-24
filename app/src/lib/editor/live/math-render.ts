import type katex from 'katex';

type Katex = typeof katex;

let katexModule: Katex | null = null;
let loading: Promise<void> | null = null;
const waiters = new Set<() => void>();

function loadMath(onReady: () => void): void {
	if (katexModule) return;
	waiters.add(onReady);
	if (loading) return;
	loading = (async () => {
		try {
			// KaTeX is browser-only: AGENTS.md requires a dynamic import so the app never boots blank.
			const [mod] = await Promise.all([import('katex'), import('katex/dist/katex.min.css')]);
			katexModule = mod.default;
		} catch (err) {
			console.warn('KaTeX failed to load:', err);
		}
		const queued = [...waiters];
		waiters.clear();
		for (const paint of queued) paint();
	})();
}

export function paintMath(target: HTMLElement, text: string, display: boolean): void {
	if (!katexModule) {
		target.textContent = text;
		loadMath(() => paintMath(target, text, display));
		return;
	}
	try {
		katexModule.render(text, target, { displayMode: display, throwOnError: true });
	} catch (err) {
		target.textContent = text;
		target.classList.add('cm-lp-math-error');
		if (err instanceof Error) target.title = err.message;
	}
}
