interface HastNode {
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

let highlighter: Promise<Highlighter | null> | null = null;

export function createHighlighter(): Promise<Highlighter | null> {
	highlighter ??= import('./lowlight')
		.then(({ common, createLowlight }) => createLowlight(common))
		.catch((err) => {
			console.warn('Code highlighting unavailable:', err);
			return null;
		});
	return highlighter;
}

// Highlighting dominates a rebuild, and a block nobody touched keeps the spans it had last time.
export class CodeSpanCache {
	private previous = new Map<string, CodeSpan[]>();
	private current = new Map<string, CodeSpan[]>();

	spans(code: Highlighter | null, language: string, text: string, offset: number): CodeSpan[] {
		if (!code) return [];
		const key = `${language}\u0000${text}`;
		let spans = this.previous.get(key);
		if (!spans) {
			spans = highlightCode(code, language, text);
			this.previous.set(key, spans);
		}
		this.current.set(key, spans);
		if (offset === 0) return spans;
		return spans.map((span) => ({
			from: span.from + offset,
			to: span.to + offset,
			className: span.className
		}));
	}

	commit(): void {
		this.previous = this.current;
		this.current = new Map();
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

function highlightCode(code: Highlighter | null, language: string, text: string): CodeSpan[] {
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
		walk(tree.children ?? [], 0, '', spans);
		return spans;
	} catch {
		return [];
	}
}
