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
	TAG,
	WIKI_LINK,
	WIKI_LINK_MARK,
	WIKI_LINK_TARGET
} from './syntax-inline';
export { BLOCK_MATH, BLOCK_MATH_MARK, COLON_CALLOUT, FRONTMATTER } from './syntax-blocks';

export const markdownSyntax = markdown({
	base: markdownLanguage,
	addKeymap: false,
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
