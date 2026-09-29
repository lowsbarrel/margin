import { WidgetType, type EditorView } from '@codemirror/view';
import * as m from '$lib/paraglide/messages.js';
import { languageLabel } from './code-languages';

function toggleTaskBox(view: EditorView, element: HTMLElement, next: boolean): void {
	const pos = view.posAtDOM(element);
	const line = view.state.doc.lineAt(pos);
	const match = /^((?:[ \t]*>)*[ \t]*(?:[-*+]|\d+[.)])[ \t]+)\[([ xX])\]/.exec(line.text);
	if (!match) return;
	const from = line.from + match[1].length + 1;
	view.dispatch({
		changes: { from, to: from + 1, insert: next ? 'x' : ' ' },
		userEvent: 'input'
	});
}

export class ListMarkWidget extends WidgetType {
	readonly text: string;
	readonly ordered: boolean;

	constructor(text: string, ordered: boolean) {
		super();
		this.text = text;
		this.ordered = ordered;
	}

	eq(other: ListMarkWidget): boolean {
		return other.text === this.text && other.ordered === this.ordered;
	}

	toDOM(): HTMLElement {
		const mark = document.createElement('span');
		mark.className = this.ordered ? 'cm-lp-number' : 'cm-lp-bullet';
		mark.textContent = this.text;
		return mark;
	}
}

export class TaskWidget extends WidgetType {
	readonly checked: boolean;

	constructor(checked: boolean) {
		super();
		this.checked = checked;
	}

	eq(other: TaskWidget): boolean {
		return other.checked === this.checked;
	}

	toDOM(view: EditorView): HTMLElement {
		const box = document.createElement('input');
		box.type = 'checkbox';
		box.className = 'cm-lp-checkbox';
		box.checked = this.checked;
		box.addEventListener('mousedown', (event) => event.preventDefault());
		box.addEventListener('click', (event) => {
			event.preventDefault();
			toggleTaskBox(view, box, box.checked);
		});
		return box;
	}

	ignoreEvent(): boolean {
		return true;
	}
}

export class RuleWidget extends WidgetType {
	eq(): boolean {
		return true;
	}

	toDOM(): HTMLElement {
		const block = document.createElement('div');
		block.className = 'cm-lp-rule';
		const rule = document.createElement('hr');
		rule.className = 'cm-lp-hr';
		block.appendChild(rule);
		return block;
	}
}

export class SubpathWidget extends WidgetType {
	eq(): boolean {
		return true;
	}

	toDOM(): HTMLElement {
		const separator = document.createElement('span');
		separator.className = 'cm-lp-subpath';
		separator.textContent = ' > ';
		return separator;
	}
}

export function strokeIcon(body: string, className: string, width = '1.8'): SVGSVGElement {
	const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
	svg.setAttribute('viewBox', '0 0 24 24');
	svg.setAttribute('fill', 'none');
	svg.setAttribute('stroke', 'currentColor');
	svg.setAttribute('stroke-width', width);
	svg.setAttribute('stroke-linecap', 'round');
	svg.setAttribute('stroke-linejoin', 'round');
	svg.setAttribute('aria-hidden', 'true');
	svg.classList.add(className);
	svg.innerHTML = body;
	return svg;
}

export function imageElement(
	doc: Document,
	image: { url: string; alt: string; width?: number; height?: number }
): HTMLImageElement {
	const element = doc.createElement('img');
	element.className = 'cm-lp-image';
	element.alt = image.alt;
	element.src = image.url;
	if (image.width) element.width = image.width;
	if (image.height) element.height = image.height;
	return element;
}

export class ImageWidget extends WidgetType {
	readonly url: string;
	readonly alt: string;
	readonly width?: number;
	readonly height?: number;

	constructor(url: string, alt: string, width?: number, height?: number) {
		super();
		this.url = url;
		this.alt = alt;
		this.width = width;
		this.height = height;
	}

	eq(other: ImageWidget): boolean {
		return (
			other.url === this.url &&
			other.alt === this.alt &&
			other.width === this.width &&
			other.height === this.height
		);
	}

	toDOM(view: EditorView): HTMLElement {
		const block = document.createElement('div');
		block.className = 'cm-lp-image-block';
		const image = imageElement(document, this);
		image.addEventListener('load', () => view.requestMeasure(), { once: true });
		block.appendChild(image);
		return block;
	}

	ignoreEvent(): boolean {
		return false;
	}
}

export class CodeHeaderWidget extends WidgetType {
	readonly language: string;
	readonly code: string;

	constructor(language: string, code: string) {
		super();
		this.language = language;
		this.code = code;
	}

	eq(other: CodeHeaderWidget): boolean {
		return other.language === this.language && other.code === this.code;
	}

	ignoreEvent(event: Event): boolean {
		return Boolean((event.target as HTMLElement | null)?.closest('.cm-lp-code-copy'));
	}

	toDOM(): HTMLElement {
		const head = document.createElement('span');
		head.className = 'cm-lp-code-head';
		const copy = document.createElement('button');
		copy.type = 'button';
		copy.className = 'cm-lp-code-copy';
		copy.title = m.sidebar_copy();
		copy.setAttribute('aria-label', m.sidebar_copy());
		copy.append(
			icon('cm-lp-icon-copy', 'M8 2h6v9M8 2H5v12h9V8'),
			icon('cm-lp-icon-done', 'M3 8.5 6 11.5 13 4.5')
		);
		copy.addEventListener('click', () => void copyCode(copy, this.code));
		head.append(copy);
		if (this.language) {
			const label = document.createElement('span');
			label.className = 'cm-lp-code-lang';
			label.textContent = languageLabel(this.language);
			head.append(label);
		}
		// Zero-size anchor: the header floats over the block so the fence line keeps its height.
		const anchor = document.createElement('span');
		anchor.className = 'cm-lp-code-tools';
		anchor.append(head);
		return anchor;
	}
}

function icon(className: string, path: string): SVGSVGElement {
	const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
	svg.setAttribute('viewBox', '0 0 16 16');
	svg.setAttribute('aria-hidden', 'true');
	svg.setAttribute('class', className);
	const shape = document.createElementNS('http://www.w3.org/2000/svg', 'path');
	shape.setAttribute('d', path);
	svg.append(shape);
	return svg;
}

async function copyCode(button: HTMLElement, code: string): Promise<void> {
	try {
		await navigator.clipboard.writeText(code);
	} catch {
		return;
	}
	button.classList.add('cm-lp-copied');
	setTimeout(() => button.classList.remove('cm-lp-copied'), 1200);
}
