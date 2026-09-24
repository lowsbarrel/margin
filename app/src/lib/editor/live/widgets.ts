import { WidgetType, type EditorView } from '@codemirror/view';

export function toggleTaskBox(view: EditorView, element: HTMLElement, next: boolean): void {
	const pos = view.posAtDOM(element);
	const line = view.state.doc.lineAt(pos);
	const match = /^([ \t]*(?:[-*+]|\d+[.)])[ \t]+)\[([ xX])\]/.exec(line.text);
	if (!match) return;
	const from = line.from + match[1].length + 1;
	view.dispatch({
		changes: { from, to: from + 1, insert: next ? 'x' : ' ' },
		userEvent: 'input'
	});
}

export class BulletWidget extends WidgetType {
	eq(): boolean {
		return true;
	}

	toDOM(): HTMLElement {
		const bullet = document.createElement('span');
		bullet.className = 'cm-lp-bullet';
		bullet.textContent = '•';
		return bullet;
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
		const rule = document.createElement('hr');
		rule.className = 'cm-lp-hr';
		return rule;
	}
}

export class ImageWidget extends WidgetType {
	readonly src: string;
	readonly alt: string;

	constructor(src: string, alt: string) {
		super();
		this.src = src;
		this.alt = alt;
	}

	eq(other: ImageWidget): boolean {
		return other.src === this.src && other.alt === this.alt;
	}

	toDOM(): HTMLElement {
		const image = document.createElement('img');
		image.className = 'cm-lp-image';
		image.alt = this.alt;
		image.src = this.src;
		return image;
	}

	ignoreEvent(): boolean {
		return false;
	}
}

export class CodeFenceWidget extends WidgetType {
	readonly language: string;

	constructor(language: string) {
		super();
		this.language = language;
	}

	eq(other: CodeFenceWidget): boolean {
		return other.language === this.language;
	}

	toDOM(): HTMLElement {
		const label = document.createElement('span');
		label.className = 'cm-lp-code-lang';
		label.textContent = this.language;
		return label;
	}
}
