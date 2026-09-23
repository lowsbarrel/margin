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
