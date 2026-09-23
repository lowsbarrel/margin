import { Channel } from '@tauri-apps/api/core';
import { commands, type AskEvent, type LlmConfig } from '$lib/bindings';

export type { ApiFormat, AskEvent, Effort, LlmConfig } from '$lib/bindings';

export async function llmConfigure(config: LlmConfig): Promise<void> {
	const result = await commands.llmConfigure(config);
	if (result.status === 'error') throw result.error;
}

export async function llmListModels(config: LlmConfig): Promise<string[]> {
	const result = await commands.llmListModels(config);
	if (result.status === 'error') throw result.error;
	return result.data;
}

// Resolves when the request ends — a provider error arrives as an event, not a throw.
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

export async function llmCancel(requestId: string): Promise<void> {
	const result = await commands.llmCancel(requestId);
	if (result.status === 'error') throw result.error;
}
