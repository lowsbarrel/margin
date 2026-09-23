/**
 * Motion preference.
 *
 * `tokens.css` already zeroes the transition durations inside
 * `prefers-reduced-motion`, but that only reaches CSS transitions. Keyframe
 * illustrations and Svelte's JS transitions never read a custom property, so
 * they ask here instead of querying `matchMedia` once per component.
 */

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
