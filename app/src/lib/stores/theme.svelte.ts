/**
 * Theme controller.
 *
 * Margin ships exactly one design system (see `$lib/styles/tokens.css`); the
 * only thing that varies is which semantic pairing is active. Setting
 * `data-theme` on `<html>` re-resolves every `--color-*` token at once, so
 * there is nothing else to apply.
 *
 * User-authored custom themes were removed deliberately: they let arbitrary
 * colours bypass the token system's contrast pairings, which is the one thing
 * a single coherent design system cannot tolerate.
 */

type BaseTheme = 'dark' | 'light';

const STORAGE_KEY = 'margin-theme';

function readStoredTheme(): BaseTheme {
	if (typeof localStorage === 'undefined') return 'dark';
	return localStorage.getItem(STORAGE_KEY) === 'light' ? 'light' : 'dark';
}

let baseTheme = $state<BaseTheme>(readStoredTheme());

function apply(t: BaseTheme) {
	if (typeof document !== 'undefined') {
		document.documentElement.setAttribute('data-theme', t);
	}
	if (typeof localStorage !== 'undefined') {
		localStorage.setItem(STORAGE_KEY, t);
	}
}

$effect.root(() => {
	apply(baseTheme);
});

export const theme = {
	get current() {
		return baseTheme;
	},

	set(t: BaseTheme) {
		if (t === baseTheme) return;
		baseTheme = t;
		apply(t);
	},

	toggle() {
		baseTheme = baseTheme === 'dark' ? 'light' : 'dark';
		apply(baseTheme);
	}
};
