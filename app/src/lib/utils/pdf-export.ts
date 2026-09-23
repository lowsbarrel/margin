import type { Editor } from '@tiptap/core';
import mermaid from 'mermaid';
import { common, createLowlight } from 'lowlight';
import { readFileBytes, saveFileBytes } from '$lib/fs/bridge';
import { toast } from '$lib/stores/toast.svelte';
import { mimeForPath } from '$lib/utils/mime';
import { save } from '@tauri-apps/plugin-dialog';
import { isLocalfileUrl, stripLocalfilePrefix } from '$lib/editor/image-url';

// getHTML() drops lowlight's decorations and leaves mermaid as source text, so both are re-rendered.
const pdfLowlight = createLowlight(common);
let pdfMermaidSeq = 0;

interface HastNode {
	type: string;
	value?: string;
	properties?: { className?: string[] | string };
	children?: HastNode[];
}

function hastToHtml(nodes: readonly HastNode[]): string {
	let out = '';
	for (const node of nodes) {
		if (node.type === 'text') {
			out += escapeHtml(node.value ?? '');
		} else if (node.type === 'element') {
			const cls = node.properties?.className;
			const className = Array.isArray(cls) ? cls.join(' ') : (cls ?? '');
			out += `<span class="${className}">${hastToHtml(node.children ?? [])}</span>`;
		}
	}
	return out;
}

function highlightCodeBlocks(container: HTMLElement): void {
	container.querySelectorAll('pre code').forEach((codeEl) => {
		const text = codeEl.textContent ?? '';
		if (!text.trim()) return;
		const lang = (codeEl.getAttribute('class') ?? '').match(/language-([\w-]+)/)?.[1];
		try {
			const tree =
				lang && pdfLowlight.registered(lang)
					? pdfLowlight.highlight(lang, text)
					: pdfLowlight.highlightAuto(text);
			codeEl.innerHTML = hastToHtml(tree.children as unknown as HastNode[]);
		} catch {}
	});
}

async function renderMermaidBlocks(container: HTMLElement): Promise<void> {
	const blocks = Array.from(container.querySelectorAll<HTMLElement>('[data-type="mermaid"]'));
	for (const el of blocks) {
		const code = el.getAttribute('data-mermaid') ?? el.textContent ?? '';
		if (!code.trim()) continue;
		try {
			// html2canvas cannot rasterize foreignObject labels, so the PDF gets SVG text labels.
			const directive = "%%{init: {'theme':'default','flowchart':{'htmlLabels':false}}}%%\n";
			const { svg } = await mermaid.render(`pdf-mmd-${++pdfMermaidSeq}`, directive + code);
			el.innerHTML = svg;
			el.removeAttribute('data-mermaid');
		} catch (err) {
			console.warn('PDF export: mermaid render failed:', err);
		}
	}
}

const PDF_STYLES = `
	#pdf-render h1 { font-size: 2rem; font-weight: 700; margin: 1.5em 0 0.5em; }
	#pdf-render h2 { font-size: 1.5rem; font-weight: 600; margin: 1.4em 0 0.4em; }
	#pdf-render h3 { font-size: 1.25rem; font-weight: 600; margin: 1.3em 0 0.3em; }
	#pdf-render p { margin: 0.5em 0; }
	#pdf-render ul, #pdf-render ol { padding-left: 1.5em; margin: 0.5em 0; }
	#pdf-render li { margin: 0.25em 0; }
	#pdf-render blockquote { border-left: 3px solid #ccc; margin: 0.5em 0; padding: 0.25em 1em; color: #555; }
	#pdf-render code { font-family: 'JetBrains Mono', monospace; font-size: 0.9em; background: #f0f0f0; padding: 0.1em 0.3em; border-radius: 3px; }
	#pdf-render pre { background: #f5f5f5; padding: 1em; border-radius: 6px; overflow-x: auto; margin: 0.5em 0; }
	#pdf-render pre code { background: none; padding: 0; }
	#pdf-render img { max-width: 100%; height: auto; }
	#pdf-render table { border-collapse: collapse; width: 100%; margin: 0.5em 0; }
	#pdf-render th, #pdf-render td { border: 1px solid #ddd; padding: 8px 12px; text-align: left; }
	#pdf-render th { background: #f5f5f5; font-weight: 600; }
	#pdf-render hr { border: none; border-top: 1px solid #ddd; margin: 1.5em 0; }
	#pdf-render a { color: #1a73e8; text-decoration: none; }
	#pdf-render mark { background: #fff3bf; padding: 0.1em 0.2em; }
	#pdf-render ul[data-type="taskList"] { list-style: none; padding-left: 0; }
	#pdf-render ul[data-type="taskList"] li { display: flex; align-items: baseline; gap: 0.5em; }
	#pdf-render .wiki-link {
		color: rgb(255, 102, 51);
		font-weight: 500;
		text-decoration: none;
		padding: 0 2px;
	}
	#pdf-render .wiki-link-icon {
		display: none;
	}
	#pdf-render .wiki-link-title {
		border-bottom: 0.05em solid rgba(255, 102, 51, 0.4);
	}
	#pdf-render .file-embed {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 10px 14px;
		margin: 6px 0;
		background: #f5f5f5;
		border: 1px solid #e0e0e0;
		border-radius: 8px;
	}
	#pdf-render .file-embed-name {
		font-size: 0.9em;
		font-weight: 500;
		color: #555;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	#pdf-render [data-type="mermaid"] { margin: 0.75em 0; text-align: center; }
	#pdf-render [data-type="mermaid"] svg { max-width: 100%; height: auto; }
	#pdf-render .callout { display: flex; gap: 10px; margin: 0.75em 0; padding: 10px 14px; border-radius: 6px; border-left: 4px solid #888; background: #f5f5f5; }
	#pdf-render .callout-indicator { flex-shrink: 0; }
	#pdf-render .callout-content { flex: 1; min-width: 0; }
	#pdf-render .callout-content > :first-child { margin-top: 0; }
	#pdf-render .callout-content > :last-child { margin-bottom: 0; }
	#pdf-render .callout-info { border-left-color: #1a73e8; background: #e8f0fe; }
	#pdf-render .callout-note { border-left-color: #6b7280; background: #f3f4f6; }
	#pdf-render .callout-success { border-left-color: #22a06b; background: #e6f4ea; }
	#pdf-render .callout-warning { border-left-color: #f59e0b; background: #fef7e6; }
	#pdf-render .callout-danger { border-left-color: #e5484d; background: #fde8e8; }
	#pdf-render .hljs-comment, #pdf-render .hljs-quote { color: #6a737d; font-style: italic; }
	#pdf-render .hljs-keyword, #pdf-render .hljs-selector-tag, #pdf-render .hljs-built_in, #pdf-render .hljs-name, #pdf-render .hljs-tag { color: #d73a49; }
	#pdf-render .hljs-string, #pdf-render .hljs-attr, #pdf-render .hljs-template-string, #pdf-render .hljs-regexp, #pdf-render .hljs-addition { color: #032f62; }
	#pdf-render .hljs-number, #pdf-render .hljs-literal, #pdf-render .hljs-variable, #pdf-render .hljs-meta { color: #005cc5; }
	#pdf-render .hljs-title, #pdf-render .hljs-section, #pdf-render .hljs-doctag { color: #6f42c1; }
	#pdf-render .hljs-type, #pdf-render .hljs-attribute, #pdf-render .hljs-symbol, #pdf-render .hljs-bullet { color: #e36209; }
	#pdf-render .hljs-emphasis { font-style: italic; }
	#pdf-render .hljs-strong { font-weight: 700; }
	#pdf-render .hljs-deletion { color: #b31d28; }
`;

async function inlineLocalImage(img: HTMLImageElement): Promise<void> {
	const src = img.getAttribute('src') ?? '';
	if (!isLocalfileUrl(src)) return;
	const tail = stripLocalfilePrefix(src) ?? '';
	const absPath = decodeURIComponent(tail);
	try {
		const bytes = await readFileBytes(absPath);
		const mime = mimeForPath(absPath);
		const blob = new Blob([bytes.buffer as ArrayBuffer], { type: mime });
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

function buildPdfContainer(title: string, html: string): HTMLDivElement {
	const container = document.createElement('div');
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

function escapeHtml(str: string): string {
	return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

export async function exportPdf(tiptap: Editor, successMessage: string): Promise<void> {
	const editorEl = tiptap.options.element;
	const el = editorEl instanceof HTMLElement ? editorEl : null;
	const title =
		el?.closest('.editor-container')?.querySelector('.title-input')?.textContent?.trim() ||
		'Document';

	const savePath = await save({
		defaultPath: `${title}.pdf`,
		filters: [{ name: 'PDF', extensions: ['pdf'] }]
	});
	if (!savePath) return;

	const html = tiptap.getHTML();
	const container = buildPdfContainer(title, html);
	document.body.appendChild(container);

	try {
		const images = container.querySelectorAll('img');
		await Promise.all(Array.from(images).map(inlineLocalImage));
		await renderMermaidBlocks(container);
		highlightCodeBlocks(container);

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
			pdf.addImage(pageCanvas.toDataURL('image/png'), 'PNG', 0, 0, imgWidth, sliceImgHeight);
		}

		const arrayBuf = pdf.output('arraybuffer');
		await saveFileBytes(savePath, new Uint8Array(arrayBuf));
		toast.success(successMessage);
	} finally {
		document.body.removeChild(container);
	}
}
