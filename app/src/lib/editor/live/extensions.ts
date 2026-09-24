import { history } from '@codemirror/commands';
import { EditorState, type Extension } from '@codemirror/state';
import {
	EditorView,
	crosshairCursor,
	drawSelection,
	dropCursor,
	highlightSpecialChars,
	placeholder,
	rectangularSelection
} from '@codemirror/view';
import * as m from '$lib/paraglide/messages.js';
import { blockWidgets } from './blocks';
import { liveClicks } from './click';
import { liveKeymap } from './commands';
import { livePaste } from './paste';
import { livePreview } from './preview';
import { markdownSyntax } from './syntax';
import { liveTheme } from './theme';

// Live-preview features live here and are switched off wholesale in raw Markdown mode.
export const previewExtensions: readonly Extension[] = [livePreview, blockWidgets, liveClicks];

// Editor behaviour that stays on in both modes.
export const baseExtensions: readonly Extension[] = [
	markdownSyntax,
	liveTheme,
	placeholder(m.editor_note_placeholder()),
	EditorView.lineWrapping,
	EditorState.tabSize.of(4),
	history(),
	drawSelection(),
	dropCursor(),
	highlightSpecialChars(),
	rectangularSelection(),
	crosshairCursor(),
	livePaste,
	liveKeymap
];
