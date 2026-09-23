import { Channel } from '@tauri-apps/api/core';
import { commands, type AskEvent, type LlmConfig } from '$lib/bindings';

export type { ApiFormat, AskEvent, Effort, LlmConfig } from '$lib/bindings';

/**
 * Hand the endpoint config to Rust, where the agent loop reads it. The API key
 * is written here once and never asked for again — the frontend holds it only
 * while the settings form is open, encrypted on disk, and in Rust state.
 */
export async function llmConfigure(config: LlmConfig): Promise<void> {
	const result = await commands.llmConfigure(config);
	if (result.status === 'error') throw result.error;
}

/** Model ids the endpoint advertises. Used by the settings form, before saving. */
export async function llmListModels(config: LlmConfig): Promise<string[]> {
	const result = await commands.llmListModels(config);
	if (result.status === 'error') throw result.error;
	return result.data;
}

/**
 * Ask the vault a question, streaming events to `onEvent` until the channel
 * closes. Resolves when the request is over — including when it failed, since a
 * provider error arrives as an `error` event so a half-streamed answer survives.
 */
export async function llmAsk(
	requestId: string,
	question: string,
	onEvent: (event: AskEvent) => void
): Promise<void> {
	const channel = new Channel<AskEvent>();
	channel.onmessage = onEvent;
	const result = await commands.llmAsk(requestId, question, channel);
	if (result.status === 'error') throw result.error;
}

/** Stop a running question. A no-op once it has finished. */
export async function llmCancel(requestId: string): Promise<void> {
	const result = await commands.llmCancel(requestId);
	if (result.status === 'error') throw result.error;
}
