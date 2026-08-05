/**
 * YAML frontmatter is split off before the document reaches TipTap and held
 * verbatim until save.
 *
 * It cannot be round-tripped through the editor: markdown-it reads the opening
 * `---` as a horizontal rule and the closing `---` as a setext H2 underline, so
 * the block comes back as a literal `## title: … tags: […]` heading in the note
 * body — and the debounced save writes that corruption to disk.
 */

export interface SplitDocument {
	/** The block including both delimiters and its trailing newline, or null. */
	frontmatter: string | null;
	body: string;
}

/** `---` alone on the very first line. */
const OPEN = /^---[ \t]*\r?\n/;

/** `---`, or YAML's `...`, alone on a line. `m` so it can match mid-document. */
const CLOSE = /^(?:---|\.\.\.)[ \t]*(?:\r?\n|$)/gm;

/**
 * The first content line of a YAML mapping is a `key:`. Requiring one is
 * stricter than the ecosystem (Obsidian and Jekyll treat any leading `---`
 * block as frontmatter) and deliberately so: it keeps a document that opens
 * with a horizontal rule above a setext heading from having that heading
 * swallowed into the hidden block. The key may be non-ASCII, so this matches
 * "anything up to a colon" rather than `\w`.
 */
const FIRST_KEY = /^[^\s:#][^:\r\n]*[ \t]*:(?:\s|$)/;

/**
 * Separate a leading frontmatter block from the note body. Slices by index, so
 * the block keeps its original line endings and spacing byte for byte.
 */
export function splitFrontmatter(markdown: string): SplitDocument {
	const whole: SplitDocument = { frontmatter: null, body: markdown };

	const open = OPEN.exec(markdown);
	if (!open) return whole;

	CLOSE.lastIndex = open[0].length;
	const close = CLOSE.exec(markdown);
	// Unterminated: the leading `---` is a horizontal rule, not a delimiter.
	if (!close) return whole;

	const inner = markdown.slice(open[0].length, close.index);
	const firstLine = inner.split('\n').find((line) => line.trim() !== '');
	if (firstLine !== undefined && !FIRST_KEY.test(firstLine)) return whole;

	const end = close.index + close[0].length;
	return { frontmatter: markdown.slice(0, end), body: markdown.slice(end) };
}

/** Re-attach a block taken by {@link splitFrontmatter} to the edited body. */
export function joinFrontmatter(frontmatter: string | null, body: string): string {
	if (frontmatter === null) return body;
	// A block that ran to EOF has no trailing newline; the body was empty then,
	// but by now the user may have typed one.
	if (body && !frontmatter.endsWith('\n')) return `${frontmatter}\n${body}`;
	return frontmatter + body;
}
