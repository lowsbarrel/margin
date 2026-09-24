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
import { liveBubble } from './bubble';
import { liveClicks } from './click';
import { liveAssist } from './complete';
import { liveContextMenu } from './context-menu';
import { liveKeymap } from './commands';
import { livePaste } from './paste';
import { livePreview } from './preview';
import { markdownSyntax } from './syntax';
import { liveTheme } from './theme';

// Live-preview features live here and are switched off wholesale in raw Markdown mode.
export const previewExtensions: readonly Extension[] = [
	liveBubble,
	livePreview,
	blockWidgets,
	liveClicks
];

// Editor behaviour that stays on in both modes.
export const baseExtensions: readonly Extension[] = [
	liveAssist,
	liveContextMenu,
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
