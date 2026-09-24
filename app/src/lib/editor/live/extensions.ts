import { history } from '@codemirror/commands';
import { EditorState, type Extension } from '@codemirror/state';
import {
	EditorView,
	crosshairCursor,
	drawSelection,
	dropCursor,
	highlightSpecialChars,
	rectangularSelection
} from '@codemirror/view';
import { blockWidgets } from './blocks';
import { liveCallouts } from './callouts';
import { liveClicks } from './click';
import { liveKeymap } from './commands';
import { liveEmbeds } from './embeds';
import { liveFootnotes } from './footnotes';
import { liveMath } from './math';
import { liveMermaid } from './mermaid';
import { livePaste } from './paste';
import { livePreview } from './preview';
import { markdownSyntax } from './syntax';
import { liveTheme } from './theme';
import './live-blocks.css';

// Live-preview features live here and are switched off wholesale in raw Markdown mode.
export const previewExtensions: readonly Extension[] = [
	livePreview,
	blockWidgets,
	liveClicks,
	liveCallouts,
	liveMath,
	liveMermaid,
	liveEmbeds,
	liveFootnotes
];

// Editor behaviour that stays on in both modes.
export const baseExtensions: readonly Extension[] = [
	markdownSyntax,
	liveTheme,
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
