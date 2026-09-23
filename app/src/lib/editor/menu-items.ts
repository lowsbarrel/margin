import type { SlashMenuItem } from './slash-command';
import { getLocale } from '$lib/paraglide/runtime.js';
import * as m from '$lib/paraglide/messages.js';

/**
 * Built per call rather than at module scope: the labels come from Paraglide,
 * which resolves them against the locale current at call time.
 */
function buildItems(): SlashMenuItem[] {
	return [
		{
			title: m.slash_text(),
			description: m.slash_text_description(),
			icon: '¶',
			searchTerms: ['p', 'paragraph', 'text'],
			command: ({ editor, range }) => {
				editor.chain().focus().deleteRange(range).toggleNode('paragraph', 'paragraph').run();
			}
		},
		{
			title: m.slash_heading1(),
			description: m.slash_heading1_description(),
			icon: 'H1',
			searchTerms: ['title', 'big', 'large', 'h1'],
			command: ({ editor, range }) => {
				editor.chain().focus().deleteRange(range).setNode('heading', { level: 1 }).run();
			}
		},
		{
			title: m.slash_heading2(),
			description: m.slash_heading2_description(),
			icon: 'H2',
			searchTerms: ['subtitle', 'medium', 'h2'],
			command: ({ editor, range }) => {
				editor.chain().focus().deleteRange(range).setNode('heading', { level: 2 }).run();
			}
		},
		{
			title: m.slash_heading3(),
			description: m.slash_heading3_description(),
			icon: 'H3',
			searchTerms: ['subtitle', 'small', 'h3'],
			command: ({ editor, range }) => {
				editor.chain().focus().deleteRange(range).setNode('heading', { level: 3 }).run();
			}
		},
		{
			title: m.slash_todo(),
			description: m.slash_todo_description(),
			icon: '☑',
			searchTerms: ['todo', 'task', 'list', 'check', 'checkbox'],
			command: ({ editor, range }) => {
				editor.chain().focus().deleteRange(range).run();

				// When inside a bullet/ordered list, indent first so that
				// toggleTaskList converts only the nested inner list instead
				// of replacing the entire parent list.
				if (editor.isActive('bulletList') || editor.isActive('orderedList')) {
					const sunk = editor.chain().sinkListItem('listItem').run();
					if (sunk) {
						editor.chain().toggleTaskList().run();
						return;
					}
				}

				editor.chain().toggleTaskList().run();
			}
		},
		{
			title: m.slash_bullet(),
			description: m.slash_bullet_description(),
			icon: '•',
			searchTerms: ['unordered', 'point', 'list', 'ul'],
			command: ({ editor, range }) => {
				editor.chain().focus().deleteRange(range).toggleBulletList().run();
			}
		},
		{
			title: m.slash_numbered(),
			description: m.slash_numbered_description(),
			icon: '1.',
			searchTerms: ['numbered', 'ordered', 'list', 'ol'],
			command: ({ editor, range }) => {
				editor.chain().focus().deleteRange(range).toggleOrderedList().run();
			}
		},
		{
			title: m.slash_quote(),
			description: m.slash_quote_description(),
			icon: '"',
			searchTerms: ['blockquote', 'quotes'],
			command: ({ editor, range }) => {
				editor.chain().focus().deleteRange(range).toggleBlockquote().run();
			}
		},
		{
			title: m.slash_code(),
			description: m.slash_code_description(),
			icon: '</>',
			searchTerms: ['codeblock', 'code', 'snippet'],
			command: ({ editor, range }) => {
				editor.chain().focus().deleteRange(range).toggleCodeBlock().run();
			}
		},
		{
			title: m.slash_divider(),
			description: m.slash_divider_description(),
			icon: '—',
			searchTerms: ['horizontal rule', 'hr', 'divider', 'separator'],
			command: ({ editor, range }) => {
				editor.chain().focus().deleteRange(range).setHorizontalRule().run();
			}
		},
		{
			title: m.slash_table(),
			description: m.slash_table_description(),
			icon: '▦',
			searchTerms: ['table', 'rows', 'columns', 'grid'],
			command: ({ editor, range }) => {
				editor
					.chain()
					.focus()
					.deleteRange(range)
					.insertTable({ rows: 3, cols: 3, withHeaderRow: true })
					.run();
			}
		},
		{
			title: m.slash_date(),
			description: m.slash_date_description(),
			icon: '📅',
			searchTerms: ['date', 'today', 'time'],
			command: ({ editor, range }) => {
				const currentDate = new Date().toLocaleDateString(getLocale(), {
					year: 'numeric',
					month: 'long',
					day: 'numeric'
				});
				editor.chain().focus().deleteRange(range).insertContent(currentDate).run();
			}
		},
		{
			title: m.slash_callout(),
			description: m.slash_callout_description(),
			icon: '💡',
			searchTerms: ['callout', 'admonition', 'alert', 'info', 'warning', 'note'],
			command: ({ editor, range }) => {
				editor.chain().focus().deleteRange(range).toggleCallout({ type: 'info' }).run();
			}
		},
		{
			title: m.slash_math_block(),
			description: m.slash_math_block_description(),
			icon: '∑',
			searchTerms: ['math', 'equation', 'formula', 'latex', 'katex', 'block'],
			command: ({ editor, range }) => {
				editor.chain().focus().deleteRange(range).setMathBlock().run();
			}
		},
		{
			title: m.slash_math_inline(),
			description: m.slash_math_inline_description(),
			icon: 'π',
			searchTerms: ['math', 'equation', 'inline', 'formula', 'latex', 'katex'],
			command: ({ editor, range }) => {
				editor.chain().focus().deleteRange(range).setMathInline().run();
			}
		},
		{
			title: m.slash_mermaid(),
			description: m.slash_mermaid_description(),
			icon: '🧜',
			searchTerms: ['mermaid', 'diagram', 'flowchart', 'graph', 'sequence', 'chart'],
			command: ({ editor, range }) => {
				editor.chain().focus().deleteRange(range).setMermaid().run();
			}
		},
		{
			title: m.slash_embed_note(),
			description: m.slash_embed_note_description(),
			icon: '🔗',
			searchTerms: ['embed', 'transclude', 'include', 'note', 'reference'],
			command: ({ editor, range }) => {
				// Insert the `![[` opener; typing the title then `]]` converts it to an
				// embed via NoteEmbed's input rule.
				editor.chain().focus().deleteRange(range).insertContent('![[').run();
			}
		}
	];
}

export function getSlashMenuItems({ query }: { query: string }): SlashMenuItem[] {
	const items = buildItems();
	if (!query) return items;
	const search = query.toLowerCase();
	return items.filter((item) => {
		return (
			item.title.toLowerCase().includes(search) ||
			item.description.toLowerCase().includes(search) ||
			item.searchTerms.some((term) => term.includes(search))
		);
	});
}
