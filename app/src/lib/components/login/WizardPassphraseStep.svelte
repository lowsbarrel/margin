<script lang="ts">
	import { KeyRound, Plus } from '@lucide/svelte';
	import GeneratedMnemonic from './GeneratedMnemonic.svelte';
	import {
		CONTROL,
		DIVIDER,
		ERROR_TEXT,
		FIELD,
		FIELD_LABEL,
		QUIET_BTN,
		STEP_BODY,
		STEP_TITLE
	} from './card-styles';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		value?: string;
		error: string;
		showGenerated: boolean;
		generatedMnemonic: string;
		copied: boolean;
		ongenerate: () => void;
		oncopy: () => void;
		onuse: () => void;
	}

	let {
		value = $bindable(''),
		error,
		showGenerated,
		generatedMnemonic,
		copied,
		ongenerate,
		oncopy,
		onuse
	}: Props = $props();
</script>

<div class="flex flex-col gap-1">
	<h2 class={STEP_TITLE}>{m.onboarding_wizard_passphrase_title()}</h2>
	<p class={STEP_BODY}>{m.onboarding_wizard_passphrase_body()}</p>
</div>
<div class="{FIELD} mt-3">
	<label for="mnemonic" class={FIELD_LABEL}>
		<KeyRound size={14} />
		{m.login_passphrase_label()}
	</label>
	<textarea
		id="mnemonic"
		bind:value
		placeholder={m.login_passphrase_placeholder()}
		rows="3"
		spellcheck="false"
		class="{CONTROL} resize-none font-mono"></textarea>
</div>

{#if error}
	<p class="{ERROR_TEXT} mt-3">{error}</p>
{/if}

<div class="{DIVIDER} mt-3"></div>

{#if showGenerated}
	<GeneratedMnemonic mnemonic={generatedMnemonic} {copied} {oncopy} {onuse} />
{:else}
	<button class="{QUIET_BTN} justify-center p-2" onclick={ongenerate}>
		<Plus size={14} />
		{m.login_generate_new()}
	</button>
{/if}
