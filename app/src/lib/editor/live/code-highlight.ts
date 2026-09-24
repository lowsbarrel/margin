export interface HastNode {
	type: string;
	value?: string;
	properties?: { className?: unknown };
	children?: HastNode[];
}

export interface Highlighter {
	highlight(language: string, value: string): HastNode | null;
	registered(name: string): boolean;
}

export interface CodeSpan {
	from: number;
	to: number;
	className: string;
}

export async function createHighlighter(): Promise<Highlighter | null> {
	try {
		const lowlight = await import('lowlight');
		return lowlight.createLowlight(lowlight.common);
	} catch (err) {
		console.warn('Code highlighting unavailable:', err);
		return null;
	}
}

function classNames(node: HastNode): string {
	const value = node.properties?.className;
	if (typeof value === 'string') return value;
	if (Array.isArray(value)) return value.filter((item) => typeof item === 'string').join(' ');
	return '';
}

function walk(nodes: HastNode[], offset: number, inherited: string, out: CodeSpan[]): number {
	let pos = offset;
	for (const node of nodes) {
		if (node.type === 'text') {
			const text = node.value ?? '';
			if (inherited && text.trim())
				out.push({ from: pos, to: pos + text.length, className: inherited });
			pos += text.length;
			continue;
		}
		if (node.type !== 'element') continue;
		const own = classNames(node);
		pos = walk(node.children ?? [], pos, own || inherited, out);
	}
	return pos;
}

export function highlightCode(
	code: Highlighter | null,
	language: string,
	text: string,
	offset: number
): CodeSpan[] {
	const lang = language
		.trim()
		.toLowerCase()
		.split(/[\s:{]/)[0];
	if (!code || !lang || !text) return [];
	try {
		if (!code.registered(lang)) return [];
		const tree = code.highlight(lang, text);
		if (!tree) return [];
		const spans: CodeSpan[] = [];
		walk(tree.children ?? [], offset, '', spans);
		return spans;
	} catch {
		return [];
	}
}
