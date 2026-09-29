export interface TextDifference {
	from: number;
	to: number;
	insert: string;
}

export function minimalDiff(oldText: string, newText: string): TextDifference {
	const shortest = Math.min(oldText.length, newText.length);
	let start = 0;
	while (start < shortest && oldText[start] === newText[start]) start++;
	let tail = 0;
	while (
		tail < shortest - start &&
		oldText[oldText.length - 1 - tail] === newText[newText.length - 1 - tail]
	) {
		tail++;
	}
	return {
		from: start,
		to: oldText.length - tail,
		insert: newText.slice(start, newText.length - tail)
	};
}
