type DiffKind = 'same' | 'added' | 'removed';

export interface DiffLine {
	kind: DiffKind;
	text: string;
	before: number | null;
	after: number | null;
}

const MAX_DIFF_LINES = 20_000;

function toLines(text: string): string[] {
	return text.split('\n').map((line) => (line.endsWith('\r') ? line.slice(0, -1) : line));
}

interface Edit {
	kind: DiffKind;
	text: string;
}

function editScript(a: string[], b: string[]): Edit[] {
	const n = a.length;
	const m = b.length;
	const max = n + m;
	const offset = max;
	const trace: number[][] = [];
	const v = new Int32Array(2 * max + 1);

	let found = -1;
	outer: for (let d = 0; d <= max; d++) {
		trace.push(Array.from(v));
		for (let k = -d; k <= d; k += 2) {
			let x: number;
			if (k === -d || (k !== d && v[offset + k - 1] < v[offset + k + 1])) {
				x = v[offset + k + 1];
			} else {
				x = v[offset + k - 1] + 1;
			}
			let y = x - k;
			while (x < n && y < m && a[x] === b[y]) {
				x++;
				y++;
			}
			v[offset + k] = x;
			if (x >= n && y >= m) {
				found = d;
				break outer;
			}
		}
	}
	if (found < 0) return [];

	const edits: Edit[] = [];
	let x = n;
	let y = m;
	for (let d = found; d >= 0; d--) {
		const prev = trace[d];
		if (d === 0) {
			while (x > 0 && y > 0) {
				x--;
				y--;
				edits.push({ kind: 'same', text: a[x] });
			}
			break;
		}
		const k = x - y;
		const down = k === -d || (k !== d && prev[offset + k - 1] < prev[offset + k + 1]);
		const prevK = down ? k + 1 : k - 1;
		const prevX = prev[offset + prevK];
		const prevY = prevX - prevK;

		while (x > prevX && y > prevY) {
			x--;
			y--;
			edits.push({ kind: 'same', text: a[x] });
		}
		if (down) {
			y--;
			edits.push({ kind: 'added', text: b[y] });
		} else {
			x--;
			edits.push({ kind: 'removed', text: a[x] });
		}
	}
	edits.reverse();
	return edits;
}

export function diffLines(before: string, after: string): DiffLine[] | null {
	const a = toLines(before);
	const b = toLines(after);
	if (a.length > MAX_DIFF_LINES || b.length > MAX_DIFF_LINES) return null;

	const edits = editScript(a, b);
	const lines: DiffLine[] = [];
	let beforeNo = 1;
	let afterNo = 1;
	for (const edit of edits) {
		if (edit.kind === 'same') {
			lines.push({ kind: 'same', text: edit.text, before: beforeNo++, after: afterNo++ });
		} else if (edit.kind === 'removed') {
			lines.push({ kind: 'removed', text: edit.text, before: beforeNo++, after: null });
		} else {
			lines.push({ kind: 'added', text: edit.text, before: null, after: afterNo++ });
		}
	}
	return lines;
}

export function countChanges(lines: DiffLine[]): { added: number; removed: number } {
	let added = 0;
	let removed = 0;
	for (const line of lines) {
		if (line.kind === 'added') added++;
		else if (line.kind === 'removed') removed++;
	}
	return { added, removed };
}
