import type { ITheme } from '@xterm/xterm';

/**
 * xterm takes concrete colours, but Margin's palette lives in CSS variables
 * that re-resolve when `data-theme` flips. A probe element reads each token
 * through the same cascade the UI uses and returns the `rgb(r, g, b)` form
 * xterm's parser reads without a canvas round-trip — the terminal then follows
 * the theme like every other surface.
 */

let probeEl: HTMLElement | null = null;

function probe(): HTMLElement {
	if (!probeEl) {
		probeEl = document.createElement('span');
		probeEl.setAttribute('aria-hidden', 'true');
		// Off-screen but in the document: `var()` only resolves for an element
		// that takes part in the cascade.
		probeEl.style.cssText = 'position:fixed;top:-100px;left:-100px;visibility:hidden';
		document.body.appendChild(probeEl);
	}
	return probeEl;
}

function tokenColor(token: string, alpha?: number): string {
	const el = probe();
	el.style.color = `var(${token})`;
	const parts = getComputedStyle(el).color.match(/[\d.]+/g);
	const [r, g, b] = parts && parts.length >= 3 ? parts.map(Number) : [0, 0, 0];
	return alpha === undefined ? `rgb(${r}, ${g}, ${b})` : `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/** Read a token as a length in pixels, for the options xterm cannot inherit. */
function tokenPixels(token: string, fallback: number): number {
	const value = parseInt(
		getComputedStyle(document.documentElement).getPropertyValue(token).trim(),
		10
	);
	return Number.isFinite(value) ? value : fallback;
}

/** The theme for the current `data-theme`, read fresh every time it changes. */
export function terminalTheme(): ITheme {
	return {
		background: tokenColor('--color-bg-primary'),
		foreground: tokenColor('--color-text-primary'),
		cursor: tokenColor('--color-text-brand'),
		cursorAccent: tokenColor('--color-bg-primary'),
		selectionBackground: tokenColor('--color-bg-brand', 0.3),
		selectionInactiveBackground: tokenColor('--color-bg-brand', 0.16),
		scrollbarSliderBackground: tokenColor('--color-text-primary', 0.2),
		scrollbarSliderHoverBackground: tokenColor('--color-text-primary', 0.4),
		scrollbarSliderActiveBackground: tokenColor('--color-text-primary', 0.5),
		// The ANSI set leans on the editor's syntax tokens so a shell and a code
		// block in a note read as the same palette.
		black: tokenColor('--color-text-tertiary'),
		red: tokenColor('--color-text-negative'),
		green: tokenColor('--color-text-positive'),
		yellow: tokenColor('--color-text-warning'),
		blue: tokenColor('--color-syntax-function'),
		magenta: tokenColor('--color-syntax-keyword'),
		cyan: tokenColor('--color-syntax-function'),
		white: tokenColor('--color-text-secondary'),
		brightBlack: tokenColor('--color-text-tertiary'),
		brightRed: tokenColor('--color-text-negative'),
		brightGreen: tokenColor('--color-text-positive'),
		brightYellow: tokenColor('--color-text-warning'),
		brightBlue: tokenColor('--color-syntax-function'),
		brightMagenta: tokenColor('--color-syntax-keyword'),
		brightCyan: tokenColor('--color-syntax-function'),
		brightWhite: tokenColor('--color-text-primary')
	};
}

/**
 * The options xterm must be told rather than inherit: it measures glyphs to
 * size the grid, so it needs the editor's monospace stack and font size.
 */
export function terminalFontOptions(): { fontFamily: string; fontSize: number } {
	const fontFamily = getComputedStyle(document.documentElement)
		.getPropertyValue('--font-mono')
		.trim();
	return {
		fontFamily: fontFamily || 'monospace',
		fontSize: tokenPixels('--text-size-xs', 12)
	};
}
