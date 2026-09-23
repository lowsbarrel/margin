const QUERY = '(prefers-reduced-motion: reduce)';

function mediaQuery(): MediaQueryList | null {
	return typeof window === 'undefined' || !window.matchMedia ? null : window.matchMedia(QUERY);
}

let reduced = $state(mediaQuery()?.matches ?? false);

mediaQuery()?.addEventListener('change', (e) => {
	reduced = e.matches;
});

export const motion = {
	get reduced() {
		return reduced;
	}
};
