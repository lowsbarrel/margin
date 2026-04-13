import type { Editor } from "@tiptap/core";
import { readFileBytes, writeFileBytes } from "$lib/fs/bridge";
import { toast } from "$lib/stores/toast.svelte";
import { mimeForPath } from "$lib/utils/mime";
import { save } from "@tauri-apps/plugin-dialog";

/**
 * PDF render styles injected into the offscreen container.
 * Keeps heavy CSS strings out of the component layer.
 */
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
`;

/** Convert a localfile:// image element to an inline base64 data URL. */
async function inlineLocalImage(img: HTMLImageElement): Promise<void> {
  const src = img.getAttribute("src") ?? "";
  if (!src.startsWith("localfile://")) return;
  const absPath = decodeURIComponent(src.replace("localfile://localhost", ""));
  try {
    const bytes = await readFileBytes(absPath);
    const mime = mimeForPath(absPath);
    const blob = new Blob([bytes.buffer as ArrayBuffer], { type: mime });
    const dataUrl = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onloadend = () => resolve(reader.result as string);
      reader.onerror = () => reject(new Error("Failed to read image blob"));
      reader.readAsDataURL(blob);
    });
    img.src = dataUrl;
  } catch (err) {
    console.warn(`PDF export: could not inline image ${absPath}:`, err);
  }
}

/** Build an offscreen DOM container with the HTML content styled for PDF. */
function buildPdfContainer(title: string, html: string): HTMLDivElement {
  const container = document.createElement("div");
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
  return str.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

/**
 * Export the current editor content as a PDF file.
 *
 * @param tiptap  The active tiptap Editor instance
 * @param successMessage  Localized success toast message
 */
export async function exportPdf(
  tiptap: Editor,
  successMessage: string,
): Promise<void> {
  const editorEl = tiptap.options.element;
  const el = editorEl instanceof HTMLElement ? editorEl : null;
  const title =
    el
      ?.closest(".editor-container")
      ?.querySelector(".title-input")
      ?.textContent?.trim() || "Document";

  const savePath = await save({
    defaultPath: `${title}.pdf`,
    filters: [{ name: "PDF", extensions: ["pdf"] }],
  });
  if (!savePath) return;

  const html = tiptap.getHTML();
  const container = buildPdfContainer(title, html);
  document.body.appendChild(container);

  try {
    // Inline local images so html2canvas can render them
    const images = container.querySelectorAll("img");
    await Promise.all(Array.from(images).map(inlineLocalImage));

    const { default: html2canvas } = await import("html2canvas");
    const { jsPDF } = await import("jspdf");

    const canvas = await html2canvas(container, {
      scale: 2,
      useCORS: true,
      backgroundColor: "#ffffff",
    });

    const imgWidth = 210; // A4 width in mm
    const pageHeight = 297; // A4 height in mm
    const imgHeight = (canvas.height * imgWidth) / canvas.width;

    const pdf = new jsPDF("p", "mm", "a4");
    let heightLeft = imgHeight;
    let position = 0;

    pdf.addImage(
      canvas.toDataURL("image/png"),
      "PNG",
      0,
      position,
      imgWidth,
      imgHeight,
    );
    heightLeft -= pageHeight;

    while (heightLeft > 0) {
      position -= pageHeight;
      pdf.addPage();
      pdf.addImage(
        canvas.toDataURL("image/png"),
        "PNG",
        0,
        position,
        imgWidth,
        imgHeight,
      );
      heightLeft -= pageHeight;
    }

    const arrayBuf = pdf.output("arraybuffer");
    await writeFileBytes(savePath, new Uint8Array(arrayBuf));
    toast.success(successMessage);
  } finally {
    document.body.removeChild(container);
  }
}
