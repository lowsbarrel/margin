<script lang="ts">
	import LogoMark from '$lib/components/LogoMark.svelte';
	import { BRAND, BRAND_TITLE } from './card-styles';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		tagline?: boolean;
	}

	let { tagline = false }: Props = $props();
</script>

<div class={BRAND}>
	<LogoMark size={80} class="mx-auto mb-3" />
	<h1 class={BRAND_TITLE}>{m.app_name()}</h1>
	{#if tagline}
		<p class="mt-1.5 font-sans text-sm text-subtle-foreground italic">{m.app_tagline()}</p>
	{/if}
</div>
