export function checkAbort(signal: AbortSignal): void {
	if (signal.aborted) throw new Error('Sync cancelled');
}
