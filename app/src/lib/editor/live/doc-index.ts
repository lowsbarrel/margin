import { syntaxTree } from '@codemirror/language';
import type { EditorState } from '@codemirror/state';
import type { SyntaxNode, Tree } from '@lezer/common';
import {
	BLOCK_MATH,
	COLON_CALLOUT,
	COMMENT,
	COMMENT_BLOCK,
	EMBED,
	FRONTMATTER,
	INLINE_MATH
} from './syntax';

const WANTED: Record<string, true> = {
	Table: true,
	Blockquote: true,
	FencedCode: true,
	HorizontalRule: true,
	Image: true,
	HTMLTag: true,
	[EMBED]: true,
	[BLOCK_MATH]: true,
	[INLINE_MATH]: true,
	[COLON_CALLOUT]: true
};

const OPAQUE: Record<string, true> = {
	FencedCode: true,
	CodeBlock: true,
	InlineCode: true,
	HTMLBlock: true,
	[BLOCK_MATH]: true,
	[INLINE_MATH]: true,
	[COMMENT]: true,
	[COMMENT_BLOCK]: true,
	[FRONTMATTER]: true
};

const indexes = new WeakMap<Tree, Map<string, SyntaxNode[]>>();

// One walk per parse tree feeds every block field; they used to walk the whole tree each.
export function nodesNamed(state: EditorState, name: string): readonly SyntaxNode[] {
	const tree = syntaxTree(state);
	let index = indexes.get(tree);
	if (!index) {
		const found = new Map<string, SyntaxNode[]>();
		tree.iterate({
			enter(ref) {
				if (WANTED[ref.name]) {
					const list = found.get(ref.name);
					if (list) list.push(ref.node);
					else found.set(ref.name, [ref.node]);
				}
				return OPAQUE[ref.name] ? false : undefined;
			}
		});
		indexes.set(tree, found);
		index = found;
	}
	return index.get(name) ?? [];
}
