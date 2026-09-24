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
import { liveBubble } from './bubble';
import { liveCallouts } from './callouts';
import { liveClicks } from './click';
import { liveAssist } from './complete';
import { liveContextMenu } from './context-menu';
import { liveKeymap } from './commands';
import { liveEmbeds } from './embeds';
import { liveFind } from './find';
import { liveFootnotes } from './footnotes';
import { liveMath } from './math';
import { liveMermaid } from './mermaid';
import { livePaste } from './paste';
import { livePreview } from './preview';
import { markdownSyntax } from './syntax';
import { liveTableEditing } from './tables';
import { liveTheme } from './theme';
import './live-blocks.css';

// Live-preview features live here and are switched off wholesale in raw Markdown mode.
export const previewExtensions: readonly Extension[] = [
	liveBubble,
	livePreview,
	blockWidgets,
	liveClicks,
	liveTableEditing,
	liveCallouts,
	liveMath,
	liveMermaid,
	liveEmbeds,
	liveFootnotes
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
	history(),
	drawSelection(),
	dropCursor(),
	highlightSpecialChars(),
	rectangularSelection(),
	crosshairCursor(),
	livePaste,
	liveKeymap,
	liveFind
];
