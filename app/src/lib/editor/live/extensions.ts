import { history } from '@codemirror/commands';
import { EditorState, type Extension } from '@codemirror/state';
import {
	EditorView,
	drawSelection,
	dropCursor,
	highlightSpecialChars,
	placeholder
} from '@codemirror/view';
import * as m from '$lib/paraglide/messages.js';
import { autoPair } from './autopair';
import { blockWidgets } from './blocks';
import { blockDrag } from './block-drag';
import { liveBubble } from './bubble';
import { liveCallouts } from './callouts';
import { liveClicks } from './click';
import { liveAssist } from './complete';
import { liveContextMenu } from './context-menu';
import { liveKeymap } from './commands';
import { liveEmbeds } from './embeds';
import { liveFind } from './find';
import { liveFootnotes } from './footnotes';
import { liveInlineHtml } from './html-inline';
import { liveMath } from './math';
import { liveMotion, selectionEscape } from './motion';
import { liveMouse } from './mouse';
import { listLines } from './list-lines';
import { liveMermaid } from './mermaid';
import { livePaste } from './paste';
import { liveProperties } from './properties';
import { livePreview } from './preview';
import { liveReveal } from './reveal';
import { markdownSyntax } from './syntax';
import { joinsCellHistory } from './table-cell';
import { liveTableEditing } from './tables';
import { liveTheme } from './theme';
import './live-blocks.css';

// Live-preview features live here and are switched off wholesale in raw Markdown mode.
export const previewExtensions: readonly Extension[] = [
	liveReveal,
	liveBubble,
	livePreview,
	listLines,
	liveInlineHtml,
	liveProperties,
	blockWidgets,
	liveClicks,
	liveTableEditing,
	liveCallouts,
	liveMath,
	liveMermaid,
	liveEmbeds,
	liveFootnotes,
	blockDrag,
	liveMotion
];

// Editor behaviour that stays on in both modes.
export const baseExtensions: readonly Extension[] = [
	liveAssist,
	liveContextMenu,
	markdownSyntax,
	liveTheme,
	placeholder(m.editor_note_placeholder()),
	EditorView.lineWrapping,
	EditorState.tabSize.of(4),
	EditorState.allowMultipleSelections.of(true),
	history({ joinToEvent: (tr, adjacent) => adjacent || joinsCellHistory(tr) }),
	drawSelection(),
	dropCursor(),
	highlightSpecialChars(),
	liveMouse,
	selectionEscape,
	livePaste,
	autoPair,
	liveKeymap,
	liveFind
];
