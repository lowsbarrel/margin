import mermaid from 'mermaid';
import { readFileBytes, saveFileBytes } from '$lib/fs/bridge';
import { toast } from '$lib/stores/toast.svelte';
import { mimeForPath } from '$lib/utils/mime';
import { save } from '@tauri-apps/plugin-dialog';
import { isLocalfileUrl, stripLocalfilePrefix } from '$lib/editor/image-url';
import { noteDir, type ResolveSources } from '$lib/editor/live/resolve';
import { contextOf } from '$lib/editor/live/context';
import { escapeHtml, renderMarkdownToHtml, type KatexLike } from '$lib/utils/pdf-markdown';
import type { EditorView } from '@codemirror/view';

let pdfMermaidSeq = 0;

const JPEG_QUALITY = 0.92;

export interface PdfTarget {
	markdown: string;
	title: string;
	host: ResolveSources;
}

export interface RenderedPdf {
	blob: Blob;
	pages: number;
	container: HTMLDivElement;
}

const PDF_STYLES = `
	.pdf-export h1 { font-size: 2rem; font-weight: 700; margin: 1.5em 0 0.5em; letter-spacing: -0.015em; }
	.pdf-export h2 { font-size: 1.5rem; font-weight: 600; margin: 1.4em 0 0.4em; letter-spacing: -0.015em; }
	.pdf-export h3 { font-size: 1.25rem; font-weight: 600; margin: 1.3em 0 0.3em; }
	.pdf-export h4 { font-size: 1.1rem; font-weight: 600; margin: 1.2em 0 0.3em; }
	.pdf-export h5, .pdf-export h6 { font-size: 1rem; font-weight: 600; margin: 1.1em 0 0.3em; color: #555; }
	.pdf-export p { margin: 0.5em 0; }
	.pdf-export ul, .pdf-export ol { padding-left: 1.5em; margin: 0.5em 0; }
	.pdf-export li { margin: 0.25em 0; }
	.pdf-export li.task-item { list-style: none; margin-left: -1.25em; }
	.pdf-export .task-checkbox { margin-right: 0.5em; vertical-align: middle; }
	.pdf-export blockquote { border-left: 3px solid #ddd; margin: 0.5em 0; padding: 0.25em 1em; color: #555; }
	.pdf-export code { font-family: 'JetBrains Mono', monospace; font-size: 0.9em; background: #f0f0f0; padding: 0.1em 0.3em; border-radius: 3px; }
	.pdf-export pre { background: #f5f5f5; padding: 1em; border-radius: 6px; overflow-x: auto; margin: 0.75em 0; }
	.pdf-export pre code { background: none; padding: 0; font-size: 0.85em; line-height: 1.5; }
	.pdf-export img { max-width: 100%; height: auto; }
	.pdf-export .wiki-embed { display: block; margin: 0.75em 0; }
	.pdf-export table { border-collapse: collapse; width: 100%; margin: 0.75em 0; }
	.pdf-export th, .pdf-export td { border: 1px solid #ddd; padding: 8px 12px; text-align: left; }
	.pdf-export th { background: #f5f5f5; font-weight: 600; }
	.pdf-export hr { border: none; border-top: 1px solid #ddd; margin: 1.5em 0; }
	.pdf-export a { color: #1a73e8; text-decoration: none; }
	.pdf-export mark { background: #fff2a8; padding: 0.1em 0.2em; border-radius: 2px; }
	.pdf-export .wiki-link { color: #d9551f; font-weight: 500; border-bottom: 1px solid rgba(217, 85, 31, 0.4); }
	.pdf-export .math-block { margin: 0.75em 0; text-align: center; overflow-x: auto; }
	.pdf-export .katex { font-size: 1.05em; }
	.pdf-export [data-type="mermaid"] { margin: 0.75em 0; text-align: center; }
	.pdf-export [data-type="mermaid"] svg { max-width: 100%; height: auto; }
	.pdf-export .callout { margin: 0.75em 0; padding: 10px 14px; border-radius: 6px; border-left: 4px solid #888; background: #f5f5f5; }
	.pdf-export .callout > :first-child { margin-top: 0; }
	.pdf-export .callout > :last-child { margin-bottom: 0; }
	.pdf-export .callout-info { border-left-color: #1a73e8; background: #e8f0fe; }
	.pdf-export .callout-note { border-left-color: #6b7280; background: #f3f4f6; }
	.pdf-export .callout-success { border-left-color: #22a06b; background: #e6f4ea; }
	.pdf-export .callout-warning { border-left-color: #f59e0b; background: #fef7e6; }
	.pdf-export .callout-danger { border-left-color: #e5484d; background: #fde8e8; }
	.pdf-export .callout-tip { border-left-color: #0d9488; background: #e6fffb; }
	.pdf-export .hljs-comment, .pdf-export .hljs-quote { color: #6a737d; font-style: italic; }
	.pdf-export .hljs-keyword, .pdf-export .hljs-selector-tag, .pdf-export .hljs-built_in, .pdf-export .hljs-name, .pdf-export .hljs-tag { color: #d73a49; }
	.pdf-export .hljs-string, .pdf-export .hljs-attr, .pdf-export .hljs-template-string, .pdf-export .hljs-regexp, .pdf-export .hljs-addition { color: #032f62; }
	.pdf-export .hljs-number, .pdf-export .hljs-literal, .pdf-export .hljs-variable, .pdf-export .hljs-meta { color: #005cc5; }
	.pdf-export .hljs-title, .pdf-export .hljs-section, .pdf-export .hljs-doctag { color: #6f42c1; }
	.pdf-export .hljs-type, .pdf-export .hljs-attribute, .pdf-export .hljs-symbol, .pdf-export .hljs-bullet { color: #e36209; }
	.pdf-export .hljs-emphasis { font-style: italic; }
	.pdf-export .hljs-strong { font-weight: 700; }
	.pdf-export .hljs-deletion { color: #b31d28; }
`;

async function loadKatex(): Promise<KatexLike> {
	// Runtime import (AGENTS.md: browser-only deps), and the stylesheet must be live before html2canvas rasterises the math.
	const [katex] = await Promise.all([import('katex'), import('katex/dist/katex.min.css')]);
	return katex.default as KatexLike;
}

async function inlineLocalImage(img: HTMLImageElement): Promise<void> {
	const src = img.getAttribute('src') ?? '';
	if (!isLocalfileUrl(src)) return;
	const tail = stripLocalfilePrefix(src) ?? '';
	const absPath = decodeURIComponent(tail);
	try {
		const bytes = await readFileBytes(absPath);
		const blob = new Blob([bytes.buffer as ArrayBuffer], { type: mimeForPath(absPath) });
		const dataUrl = await new Promise<string>((resolve, reject) => {
			const reader = new FileReader();
			reader.onloadend = () => resolve(reader.result as string);
			reader.onerror = () => reject(new Error('Failed to read image blob'));
			reader.readAsDataURL(blob);
		});
		img.src = dataUrl;
	} catch (err) {
		console.warn(`PDF export: could not inline image ${absPath}:`, err);
	}
}

async function renderMermaidBlocks(container: HTMLElement): Promise<void> {
	const blocks = Array.from(container.querySelectorAll<HTMLElement>('[data-type="mermaid"]'));
	for (const el of blocks) {
		const code = el.getAttribute('data-mermaid') ?? el.textContent ?? '';
		if (!code.trim()) continue;
		try {
			// html2canvas cannot rasterize foreignObject labels, so the PDF gets SVG text labels.
			const directive =
				"%%{init: {'theme':'default','look':'classic','layout':'dagre','flowchart':{'htmlLabels':false}}}%%\n";
			const { svg } = await mermaid.render(`pdf-mmd-${++pdfMermaidSeq}`, directive + code);
			el.innerHTML = svg;
			el.removeAttribute('data-mermaid');
		} catch (err) {
			console.warn('PDF export: mermaid render failed:', err);
		}
	}
}

export function buildExportContainer(title: string, html: string): HTMLDivElement {
	const container = document.createElement('div');
	container.className = 'pdf-export';
	container.style.cssText = `
		position: fixed; left: -9999px; top: 0;
		width: 700px; padding: 40px;
		font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
		font-size: 14px; line-height: 1.6; color: #1a1a1a;
		background: #fff;
	`;
	container.innerHTML = `
		<div style="font-size:2.25rem;font-weight:700;margin-bottom:1em;letter-spacing:-0.02em">${escapeHtml(title)}</div>
		<style>${PDF_STYLES}</style>
		<div id="pdf-render">${html}</div>
	`;
	return container;
}

export async function renderPdf(target: PdfTarget): Promise<RenderedPdf> {
	const katex = await loadKatex();
	const html = renderMarkdownToHtml(target.markdown, { host: target.host, katex });
	const container = buildExportContainer(target.title, html);
	document.body.appendChild(container);
	try {
		await Promise.all(Array.from(container.querySelectorAll('img')).map(inlineLocalImage));
		await renderMermaidBlocks(container);

		const { default: html2canvas } = await import('html2canvas-pro');
		const { jsPDF } = await import('jspdf');

		const canvas = await html2canvas(container, {
			scale: 2,
			useCORS: true,
			backgroundColor: '#ffffff'
		});

		const imgWidth = 210;
		const pageHeight = 297;
		const pdf = new jsPDF('p', 'mm', 'a4');
		const pxPerPage = Math.floor((pageHeight * canvas.width) / imgWidth);
		const totalPages = Math.max(1, Math.ceil(canvas.height / pxPerPage));

		for (let page = 0; page < totalPages; page++) {
			const sliceY = page * pxPerPage;
			const sliceHeight = Math.min(pxPerPage, canvas.height - sliceY);
			const pageCanvas = document.createElement('canvas');
			pageCanvas.width = canvas.width;
			pageCanvas.height = sliceHeight;
			const ctx = pageCanvas.getContext('2d');
			if (!ctx) continue;
			ctx.fillStyle = '#ffffff';
			ctx.fillRect(0, 0, pageCanvas.width, pageCanvas.height);
			ctx.drawImage(canvas, 0, sliceY, canvas.width, sliceHeight, 0, 0, canvas.width, sliceHeight);
			const sliceImgHeight = (sliceHeight * imgWidth) / canvas.width;
			if (page > 0) pdf.addPage();
			pdf.addImage(
				pageCanvas.toDataURL('image/jpeg', JPEG_QUALITY),
				'JPEG',
				0,
				0,
				imgWidth,
				sliceImgHeight,
				undefined,
				'FAST'
			);
		}

		return { blob: pdf.output('blob'), pages: pdf.getNumberOfPages(), container };
	} catch (err) {
		container.remove();
		throw err;
	}
}

export async function exportPdf(view: EditorView, successMessage: string): Promise<void> {
	const ctx = contextOf(view.state);
	const titleElement = view.dom.closest('.editor-container')?.querySelector('.title-input');
	const title = titleElement?.textContent?.trim() || 'Document';

	const savePath = await save({
		defaultPath: `${title}.pdf`,
		filters: [{ name: 'PDF', extensions: ['pdf'] }]
	});
	if (!savePath) return;

	const target: PdfTarget = {
		markdown: view.state.doc.toString(),
		title,
		host: {
			vaultPath: ctx.vaultPath,
			noteDir: () => noteDir(ctx.notePath(), ctx.vaultPath()),
			attachmentFolder: ctx.attachmentFolder,
			exists: ctx.exists,
			findByName: ctx.findByName
		}
	};

	let rendered: RenderedPdf | null = null;
	try {
		rendered = await renderPdf(target);
		await saveFileBytes(savePath, new Uint8Array(await rendered.blob.arrayBuffer()));
		toast.success(successMessage);
	} finally {
		rendered?.container.remove();
	}
}
