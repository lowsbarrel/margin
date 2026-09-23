import { TextLayer, type PDFDocumentProxy } from 'pdfjs-dist';

export interface PageSlot {
	num: number;
	width: number;
	height: number;
}

export interface PageRenderDeps {
	document: () => PDFDocumentProxy | null;
	scale: () => number;
	onMeasured: (num: number, width: number, height: number) => void;
	onLayerReady: (num: number, layer: TextLayer) => void;
	onLayerGone: (num: number) => void;
}

const MEASURE_TOLERANCE_PX = 0.5;

export function createPageActions(deps: PageRenderDeps) {
	function renderPage(canvas: HTMLCanvasElement, slot: PageSlot) {
		let cancelled = false;

		(async () => {
			const pdf = deps.document();
			if (!pdf) return;
			const page = await pdf.getPage(slot.num);
			if (cancelled) return;
			const viewport = page.getViewport({ scale: deps.scale() });
			const ratio = window.devicePixelRatio || 1;
			canvas.width = Math.floor(viewport.width * ratio);
			canvas.height = Math.floor(viewport.height * ratio);
			canvas.style.width = `${viewport.width}px`;
			canvas.style.height = `${viewport.height}px`;
			await page.render({
				canvas,
				viewport,
				transform: ratio === 1 ? undefined : [ratio, 0, 0, ratio, 0, 0]
			}).promise;
			const unscaled = page.getViewport({ scale: 1 });
			if (cancelled) return;
			if (
				Math.abs(slot.width - unscaled.width) > MEASURE_TOLERANCE_PX ||
				Math.abs(slot.height - unscaled.height) > MEASURE_TOLERANCE_PX
			) {
				deps.onMeasured(slot.num, unscaled.width, unscaled.height);
			}
		})().catch((err) => {
			if (!cancelled) console.error(`Failed to render PDF page ${slot.num}:`, err);
		});

		return {
			destroy() {
				cancelled = true;
				canvas.width = 0;
				canvas.height = 0;
			}
		};
	}

	function textLayer(el: HTMLDivElement, pageNum: number) {
		let cancelled = false;
		let layer: TextLayer | null = null;

		(async () => {
			const pdf = deps.document();
			if (!pdf) return;
			const page = await pdf.getPage(pageNum);
			if (cancelled) return;
			const viewport = page.getViewport({ scale: deps.scale() });
			const content = await page.getTextContent();
			if (cancelled) return;
			layer = new TextLayer({ textContentSource: content, container: el, viewport });
			await layer.render();
			if (cancelled) {
				layer.cancel();
				return;
			}
			deps.onLayerReady(pageNum, layer);
		})().catch((err) => {
			if (!cancelled) console.error(`Failed to lay out PDF text for page ${pageNum}:`, err);
		});

		return {
			destroy() {
				cancelled = true;
				deps.onLayerGone(pageNum);
				layer?.cancel();
				el.replaceChildren();
			}
		};
	}

	return { renderPage, textLayer };
}
