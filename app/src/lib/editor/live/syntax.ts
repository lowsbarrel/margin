import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
import { GFM } from '@lezer/markdown';
import { Embed, Highlight, InlineMath, Tag, WikiLink } from './syntax-inline';
import { BlockMath, ColonCallout, Frontmatter } from './syntax-blocks';

export {
	EMBED,
	EMBED_MARK,
	HIGHLIGHT,
	HIGHLIGHT_MARK,
	INLINE_MATH,
	INLINE_MATH_MARK,
	TAG,
	WIKI_LINK,
	WIKI_LINK_MARK,
	WIKI_LINK_TARGET
} from './syntax-inline';
export {
	BLOCK_MATH,
	BLOCK_MATH_MARK,
	COLON_CALLOUT,
	COLON_CALLOUT_MARK,
	FRONTMATTER,
	FRONTMATTER_MARK
} from './syntax-blocks';

export const markdownSyntax = markdown({
	base: markdownLanguage,
	extensions: [
		GFM,
		WikiLink,
		Embed,
		Highlight,
		Tag,
		InlineMath,
		BlockMath,
		ColonCallout,
		Frontmatter
	]
});
