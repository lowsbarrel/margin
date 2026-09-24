import { readFileBytes } from '$lib/fs/bridge';

export function noteHeadings(text: string): string[] {
	const headings: string[] = [];
	let fenced = false;
	let frontmatter = false;
	text.split('\n').forEach((raw, index) => {
		const line = raw.replace(/\r$/, '');
		if (index === 0 && line.trim() === '---') {
			frontmatter = true;
			return;
		}
		if (frontmatter) {
			if (/^(---|\.\.\.)[ \t]*$/.test(line)) frontmatter = false;
			return;
		}
		if (/^[ \t]*(```|~~~)/.test(line)) {
			fenced = !fenced;
			return;
		}
		if (fenced) return;
		const match = /^#{1,6}[ \t]+(.+?)[ \t]*#*[ \t]*$/.exec(line);
		const title = match?.[1]?.trim();
		if (title) headings.push(title);
	});
	return headings;
}

export async function headingsOfNote(absPath: string): Promise<string[]> {
	try {
		const bytes = await readFileBytes(absPath);
		return noteHeadings(new TextDecoder().decode(bytes));
	} catch {
		return [];
	}
}
