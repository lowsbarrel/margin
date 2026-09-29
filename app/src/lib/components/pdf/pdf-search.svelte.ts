import { tick } from 'svelte';
import type { PDFDocumentProxy, TextLayer } from 'pdfjs-dist';

interface PageText {
	text: string;
	starts: number[];
}

interface PdfMatch {
	page: number;
	start: number;
	end: number;
}

export interface PdfSearchDeps {
	document: () => PDFDocumentProxy | null;
	scrollToPage: (page: number) => void;
}

function sleep(ms: number): Promise<void> {
	const { promise, resolve } = Promise.withResolvers<void>();
	setTimeout(resolve, ms);
	return promise;
}

export class PdfSearch {
	query = $state('');
	matches = $state.raw<PdfMatch[]>([]);
	matchIndex = $state(0);
	searching = $state(false);
	markedPage: number | null = null;

	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	private layers = new Map<number, TextLayer>();
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	private texts = new Map<number, PageText>();
	private token = 0;

	constructor(private readonly deps: PdfSearchDeps) {}

	async run() {
		const needle = this.query.trim().toLowerCase();
		const token = ++this.token;
		this.matchIndex = 0;
		const pdf = this.deps.document();
		if (!needle || !pdf) {
			this.matches = [];
			this.searching = false;
			return;
		}
		this.searching = true;
		const found: PdfMatch[] = [];
		for (let page = 1; page <= pdf.numPages; page++) {
			let text: PageText;
			try {
				text = await this.pageText(page);
			} catch {
				continue;
			}
			if (token !== this.token) return;
			const haystack = text.text.toLowerCase();
			let at = haystack.indexOf(needle);
			while (at >= 0) {
				found.push({ page, start: at, end: at + needle.length });
				at = haystack.indexOf(needle, at + needle.length);
			}
			this.matches = [...found];
		}
		this.searching = false;
		if (found.length > 0) await this.goTo(0);
	}

	async goTo(index: number) {
		if (this.matches.length === 0) return;
		const at = ((index % this.matches.length) + this.matches.length) % this.matches.length;
		if (this.markedPage !== null) this.clearMarks(this.markedPage);
		this.matchIndex = at;
		this.markedPage = null;
		const match = this.matches[at];
		this.deps.scrollToPage(match.page);
		await tick();
		await this.awaitLayer(match.page);
		this.markMatch(match.page, match);
	}

	registerLayer(pageNum: number, layer: TextLayer) {
		this.layers.set(pageNum, layer);
		// A page that scrolls back into view repaints without its highlight.
		const current = this.matches[this.matchIndex];
		if (current?.page === pageNum) this.markMatch(pageNum, current);
	}

	forgetLayer(pageNum: number) {
		this.layers.delete(pageNum);
	}

	cancel() {
		this.token++;
	}

	private pageText(pageNum: number): Promise<PageText> {
		const cached = this.texts.get(pageNum);
		if (cached) return Promise.resolve(cached);
		return this.deps
			.document()!
			.getPage(pageNum)
			.then((page) =>
				page.getTextContent().then((content) => {
					const starts: number[] = [];
					let text = '';
					for (const item of content.items) {
						if (!('str' in item)) continue;
						starts.push(text.length);
						text += item.str;
					}
					const entry = { text, starts };
					this.texts.set(pageNum, entry);
					return entry;
				})
			);
	}

	private async awaitLayer(pageNum: number): Promise<TextLayer | null> {
		for (let attempt = 0; attempt < 40; attempt++) {
			const layer = this.layers.get(pageNum);
			if (layer) return layer;
			await sleep(25);
		}
		return null;
	}

	private clearMarks(pageNum: number) {
		const layer = this.layers.get(pageNum);
		if (!layer) return;
		for (const div of layer.textDivs) {
			const marks = Array.from(div.querySelectorAll('.pdf-find-hit'));
			if (marks.length === 0) continue;
			for (const mark of marks) mark.replaceWith(...Array.from(mark.childNodes));
			div.normalize();
		}
	}

	private markMatch(pageNum: number, match: PdfMatch) {
		const layer = this.layers.get(pageNum);
		const text = this.texts.get(pageNum);
		if (!layer || !text) return;
		// The layer's spans follow the text items, so an item's offset is its span index.
		let startSpan = 0;
		while (startSpan + 1 < text.starts.length && text.starts[startSpan + 1] <= match.start) {
			startSpan++;
		}
		let endSpan = startSpan;
		while (endSpan + 1 < text.starts.length && text.starts[endSpan + 1] < match.end) endSpan++;
		const from = layer.textDivs[startSpan];
		const to = layer.textDivs[endSpan];
		if (!from?.firstChild || !to?.firstChild) return;

		const range = document.createRange();
		range.setStart(
			from.firstChild,
			Math.min(match.start - text.starts[startSpan], from.textContent?.length ?? 0)
		);
		range.setEnd(
			to.firstChild,
			Math.min(match.end - text.starts[endSpan], to.textContent?.length ?? 0)
		);
		const mark = document.createElement('span');
		mark.className = 'pdf-find-hit';
		range.surroundContents(mark);
		this.markedPage = pageNum;
		mark.scrollIntoView({ block: 'center' });
	}
}
