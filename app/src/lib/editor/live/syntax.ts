import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
import { GFM } from '@lezer/markdown';
import { Comment, Embed, Highlight, InlineMath, Tag, WikiLink } from './syntax-inline';
import { BlockMath, ColonCallout, CommentBlock, Frontmatter } from './syntax-blocks';

export {
	COMMENT,
	EMBED,
	EMBED_MARK,
	HIGHLIGHT,
	HIGHLIGHT_MARK,
	INLINE_MATH,
	TAG,
	WIKI_LINK,
	WIKI_LINK_MARK,
	WIKI_LINK_TARGET
} from './syntax-inline';
export {
	BLOCK_MATH,
	BLOCK_MATH_MARK,
	COLON_CALLOUT,
	COMMENT_BLOCK,
	FRONTMATTER
} from './syntax-blocks';

export const markdownSyntax = markdown({
	base: markdownLanguage,
	addKeymap: false,
	pasteURLAsLink: false,
	extensions: [
		GFM,
		WikiLink,
		Embed,
		Highlight,
		Tag,
		InlineMath,
		Comment,
		BlockMath,
		ColonCallout,
		CommentBlock,
		Frontmatter
	]
});

// A GFM table cell holds inline content only, so its editor must not see list, heading or quote markers.
export const cellSyntax = markdown({
	base: markdownLanguage,
	addKeymap: false,
	pasteURLAsLink: false,
	extensions: [
		WikiLink,
		Embed,
		Highlight,
		Tag,
		InlineMath,
		Comment,
		{
			remove: [
				'LinkReference',
				'IndentedCode',
				'FencedCode',
				'Blockquote',
				'HorizontalRule',
				'BulletList',
				'OrderedList',
				'ATXHeading',
				'HTMLBlock',
				'SetextHeading',
				'Table',
				'TaskList'
			]
		}
	]
});
