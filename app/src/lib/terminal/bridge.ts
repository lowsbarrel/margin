import { Channel } from '@tauri-apps/api/core';
import { commands } from '$lib/bindings';

export interface TerminalStream {
	onOutput: (text: string) => void;
	onExit: (code: number) => void;
}

export async function ptySpawn(
	id: number,
	cols: number,
	rows: number,
	stream: TerminalStream
): Promise<void> {
	const result = await commands.ptySpawn(
		id,
		cols,
		rows,
		new Channel<string>((text) => stream.onOutput(text)),
		new Channel<number>((code) => stream.onExit(code))
	);
	if (result.status === 'error') throw new Error(result.error);
}

export async function ptyWrite(id: number, data: string): Promise<void> {
	const result = await commands.ptyWrite(id, data);
	if (result.status === 'error') throw new Error(result.error);
}

export async function ptyResize(id: number, cols: number, rows: number): Promise<void> {
	const result = await commands.ptyResize(id, cols, rows);
	if (result.status === 'error') throw new Error(result.error);
}

export async function ptyKill(id: number): Promise<void> {
	const result = await commands.ptyKill(id);
	if (result.status === 'error') throw new Error(result.error);
}

export async function ptyKillAll(): Promise<void> {
	const result = await commands.ptyKillAll();
	if (result.status === 'error') throw new Error(result.error);
}
