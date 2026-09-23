/**
 * Line diff for the history panel: what a snapshot holds against what the file
 * currently holds.
 *
 * A Myers O(ND) walk with linear-space tracing, because the alternative — an
 * LCS table — allocates rows × columns for two versions of the same note, and
 * notes are exactly the files people edit for months. The walk is bounded: past
 * `MAX_DIFF_LINES` on either side the panel shows the snapshot alone rather than
 * spending seconds diffing a file no one wants to read line by line.
 */

export type DiffKind = 'same' | 'added' | 'removed';

export interface DiffLine {
	kind: DiffKind;
	text: string;
	/** 1-based line number in `before`; null for an added line. */
	before: number | null;
	/** 1-based line number in `after`; null for a removed line. */
	after: number | null;
}

/** Beyond this many lines on either side, the diff is not attempted. */
export const MAX_DIFF_LINES = 20_000;

/** Lines with their trailing CR stripped, so CRLF and LF files compare equal. */
function toLines(text: string): string[] {
	return text.split('\n').map((line) => (line.endsWith('\r') ? line.slice(0, -1) : line));
}

interface Edit {
	kind: DiffKind;
	text: string;
}

/**
 * The shortest edit script between `a` and `b` (Myers, forward greedy with a
 * per-step trace). Classic middle-snake search; the trace is replayed backwards
 * to emit the edits in order.
 */
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
			// The run of common lines from the origin that opened the search.
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

		// The snake that ended this step, then the single edit that started it.
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

/**
 * Line diff of `before` (the snapshot) against `after` (the current file).
 * Returns null when either side is too large to diff.
 */
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

/** How many lines differ, for the panel's summary. */
export function countChanges(lines: DiffLine[]): { added: number; removed: number } {
	let added = 0;
	let removed = 0;
	for (const line of lines) {
		if (line.kind === 'added') added++;
		else if (line.kind === 'removed') removed++;
	}
	return { added, removed };
}
