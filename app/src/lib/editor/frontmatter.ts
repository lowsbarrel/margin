export interface SplitDocument {
	frontmatter: string | null;
	body: string;
}

// markdown-it reads the fences as an hr plus a setext H2 underline and corrupts the block on save.
const OPEN = /^---[ \t]*\r?\n/;

const CLOSE = /^(?:---|\.\.\.)[ \t]*(?:\r?\n|$)/gm;

const FIRST_KEY = /^[^\s:#][^:\r\n]*[ \t]*:(?:\s|$)/;

export function splitFrontmatter(markdown: string): SplitDocument {
	const whole: SplitDocument = { frontmatter: null, body: markdown };

	const open = OPEN.exec(markdown);
	if (!open) return whole;

	CLOSE.lastIndex = open[0].length;
	const close = CLOSE.exec(markdown);
	if (!close) return whole;

	const inner = markdown.slice(open[0].length, close.index);
	const firstLine = inner.split('\n').find((line) => line.trim() !== '');
	if (firstLine !== undefined && !FIRST_KEY.test(firstLine)) return whole;

	const end = close.index + close[0].length;
	return { frontmatter: markdown.slice(0, end), body: markdown.slice(end) };
}

export function joinFrontmatter(frontmatter: string | null, body: string): string {
	if (frontmatter === null) return body;
	if (body && !frontmatter.endsWith('\n')) return `${frontmatter}\n${body}`;
	return frontmatter + body;
}
