import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { EditorView } from '@codemirror/view';
import { tags } from '@lezer/highlight';

const SELECTION =
	'&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection';

export const liveTheme = EditorView.theme({
	'&': {
		height: 'auto',
		backgroundColor: 'transparent',
		color: 'var(--color-text-primary)',
		fontFamily: 'var(--font-sans)',
		fontSize: '15px'
	},
	'&.cm-focused': { outline: 'none' },
	'.cm-scroller': {
		overflow: 'visible',
		fontFamily: 'inherit',
		lineHeight: '1.6'
	},
	'.cm-content': {
		padding: '12px 2.5rem 50vh',
		// Same column as the inline title's `max-w-[750px]`; a rem value would drift with the root size.
		maxWidth: '750px',
		margin: '0 auto',
		width: '100%',
		cursor: 'text',
		caretColor: 'var(--color-text-primary)',
		userSelect: 'text'
	},
	'.cm-line': { padding: '0' },
	'.cm-placeholder': { color: 'var(--color-text-tertiary)' },
	'.cm-gutters': { display: 'none' },
	'.cm-activeLine': { backgroundColor: 'transparent' },
	'.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--color-text-primary)' },
	[SELECTION]: { backgroundColor: 'var(--color-brand-24)' },
	'.cm-panels': { backgroundColor: 'var(--color-bg-secondary)', color: 'var(--color-text-primary)' }
});

export const sourceHighlighting = syntaxHighlighting(
	HighlightStyle.define([
		{ tag: tags.heading, color: 'var(--color-text-primary)', fontWeight: '700' },
		{ tag: tags.strong, fontWeight: '600' },
		{ tag: tags.emphasis, fontStyle: 'italic' },
		{ tag: tags.strikethrough, textDecoration: 'line-through' },
		{ tag: tags.link, color: 'var(--color-text-brand)' },
		{ tag: tags.url, color: 'var(--color-text-tertiary)' },
		{ tag: tags.monospace, color: 'var(--color-syntax-attr)' },
		{ tag: tags.processingInstruction, color: 'var(--color-text-tertiary)' },
		{ tag: tags.contentSeparator, color: 'var(--color-text-tertiary)' },
		{ tag: tags.quote, color: 'var(--color-text-secondary)' },
		{ tag: tags.atom, color: 'var(--color-text-brand)' },
		{ tag: tags.special(tags.content), backgroundColor: 'var(--color-highlight-yellow)' }
	])
);
