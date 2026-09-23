import { llmAsk, llmCancel, llmConfigure, type AskEvent } from '$lib/ai/bridge';
import { loadSettings } from '$lib/settings/bridge';
import { vault } from '$lib/stores/vault.svelte';

/** One tool invocation the answer used, as shown in the palette trace. */
export interface AskStep {
	name: string;
	summary: string;
}

interface AskState {
	steps: AskStep[];
	answer: string;
	error: string | null;
	running: boolean;
	/** `null` until the config has been checked for the current vault. */
	configured: boolean | null;
}

const state = $state<AskState>({
	steps: [],
	answer: '',
	error: null,
	running: false,
	configured: null
});

let requestId: string | null = null;
/** The vault whose LLM config has already been handed to Rust state. */
let configuredFor: string | null = null;

/**
 * Point Rust at the vault's saved endpoint, once per vault.
 *
 * The config lives in state rather than being passed with each question so the
 * key crosses the IPC boundary on settings load rather than on every ask.
 */
async function ensureConfigured(): Promise<boolean> {
	if (!vault.vaultPath || !vault.encryptionKey) {
		state.configured = false;
		return false;
	}
	const root = vault.vaultPath;
	if (state.configured === true && configuredFor === root) return true;

	try {
		const settings = await loadSettings(root, vault.encryptionKey);
		if (vault.vaultPath !== root) return false;
		if (!settings?.llm) {
			state.configured = false;
			configuredFor = null;
			return false;
		}
		await llmConfigure(settings.llm);
		state.configured = true;
		configuredFor = root;
		return true;
	} catch (err) {
		console.warn('Failed to configure AI:', err);
		state.configured = false;
		return false;
	}
}

function apply(event: AskEvent) {
	switch (event.type) {
		case 'tool':
			state.steps.push({ name: event.name, summary: event.summary });
			break;
		case 'delta':
			state.answer += event.text;
			break;
		case 'done':
			state.running = false;
			break;
		case 'error':
			state.error = event.message;
			state.running = false;
			break;
	}
}

async function sendQuestion(question: string): Promise<void> {
	const trimmed = question.trim();
	if (!trimmed || state.running) return;

	state.steps = [];
	state.answer = '';
	state.error = null;
	state.running = true;

	const id = crypto.randomUUID();
	requestId = id;
	try {
		await llmAsk(id, trimmed, (event) => {
			// Ignore events from a request the user has already abandoned: a
			// cancelled stream keeps delivering for a moment.
			if (requestId !== id) return;
			apply(event);
		});
	} catch (err) {
		if (requestId === id) state.error = String(err);
	} finally {
		// The id is kept, not cleared: the channel's last deltas can arrive
		// after the command's promise resolves, and dropping them would truncate
		// the answer. Only `cancel` abandons an id.
		if (requestId === id) state.running = false;
	}
}

/** Esc: stop the stream. The partial answer stays on screen. */
async function cancel(): Promise<void> {
	const id = requestId;
	if (!id) return;
	requestId = null;
	state.running = false;
	try {
		await llmCancel(id);
	} catch (err) {
		console.warn('Failed to cancel the question:', err);
	}
}

function reset(): void {
	state.steps = [];
	state.answer = '';
	state.error = null;
}

/** Called by the settings form once it has pushed (or removed) a config. */
function markConfigured(configured: boolean): void {
	state.configured = configured;
	configuredFor = configured ? vault.vaultPath : null;
}

export const ask = {
	get steps() {
		return state.steps;
	},
	get answer() {
		return state.answer;
	},
	get error() {
		return state.error;
	},
	get running() {
		return state.running;
	},
	get configured() {
		return state.configured;
	},
	ensureConfigured,
	ask: sendQuestion,
	cancel,
	reset,
	markConfigured
};
