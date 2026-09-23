<script lang="ts">
	import { Plus, Play, Trash2 } from '@lucide/svelte';
	import type { VaultProfile } from '$lib/session/bridge';
	import { CARD_WITHOUT_GAP, DIVIDER, ERROR_TEXT, QUIET_BTN } from './card-styles';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		profiles: VaultProfile[];
		error: string;
		onselect: (profile: VaultProfile) => void;
		onremove: (profile: VaultProfile) => void;
		onadd: () => void;
		onreplay: () => void;
	}

	let { profiles, error, onselect, onremove, onadd, onreplay }: Props = $props();
</script>

<div class="{CARD_WITHOUT_GAP} gap-2">
	<p class="mb-1 font-sans text-sm font-medium text-subtle-foreground">{m.login_choose_vault()}</p>
	{#each profiles as profile (profile.vault_path)}
		<div
			class="flex w-full cursor-pointer items-center justify-between rounded-sm border border-border bg-surface-2 px-3.5 py-2.5 text-left transition-colors hover:border-subtle-foreground hover:bg-surface-1"
			onclick={() => onselect(profile)}
			role="button"
			tabindex="0"
			onkeydown={(e: KeyboardEvent) => {
				if (e.key === 'Enter') onselect(profile);
			}}
		>
			<div class="flex flex-col gap-0.5 overflow-hidden">
				<span class="font-sans text-sm font-medium text-foreground">{profile.name}</span>
				<span class="truncate font-mono text-xs text-subtle-foreground">{profile.vault_path}</span>
			</div>
			<button
				class="flex size-7 shrink-0 items-center justify-center rounded-xs bg-transparent p-0 text-subtle-foreground transition-colors hover:text-destructive"
				onclick={(e: MouseEvent) => {
					e.stopPropagation();
					onremove(profile);
				}}
				title={m.login_remove_vault()}
			>
				<Trash2 size={14} />
			</button>
		</div>
	{/each}

	{#if error}
		<p class={ERROR_TEXT}>{error}</p>
	{/if}

	<div class={DIVIDER}></div>
	<button class="{QUIET_BTN} justify-center p-2" onclick={onadd}>
		<Plus size={14} />
		{m.login_add_vault()}
	</button>
	<button class="{QUIET_BTN} justify-center p-2" onclick={onreplay}>
		<Play size={14} />
		{m.onboarding_replay()}
	</button>
</div>
