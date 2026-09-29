<script lang="ts">
	import { Check, Copy } from '@lucide/svelte';
	import { Button } from '$lib/ui';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		mnemonic: string;
		copied: boolean;
		oncopy: () => void;
		onuse: () => void;
	}

	let { mnemonic, copied, oncopy, onuse }: Props = $props();
</script>

<div class="flex flex-col gap-3">
	<p class="font-sans text-sm text-muted-foreground italic">{m.login_generated_hint()}</p>
	<div
		class="relative rounded-sm border border-dashed border-border bg-surface-2 p-3 pr-10 font-mono text-sm leading-[1.6] text-foreground [word-spacing:0.3em]"
	>
		<p>{mnemonic}</p>
		<button
			class="absolute top-2 right-2 flex bg-transparent p-1 text-subtle-foreground transition-colors hover:text-foreground"
			onclick={oncopy}
			aria-label={m.login_copy_passphrase()}
		>
			{#if copied}
				<Check size={14} />
			{:else}
				<Copy size={14} />
			{/if}
		</button>
	</div>
	<Button variant="primary" size="sm" fullWidth onclick={onuse}>
		{m.login_use_passphrase()}
	</Button>
</div>
