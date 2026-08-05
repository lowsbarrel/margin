<script lang="ts">
	import { Section } from '$lib/ui';
	import { Globe } from '@lucide/svelte';
	import { getLocale, setLocale, locales, type Locale } from '$lib/paraglide/runtime.js';
	import * as m from '$lib/paraglide/messages.js';

	function handleLocaleChange(e: Event) {
		const target = e.target as HTMLSelectElement;
		if ((locales as readonly string[]).includes(target.value)) {
			setLocale(target.value as Locale);
		}
	}
</script>

<Section title={m.settings_language()} icon={Globe} collapsible defaultOpen={false}>
	<!-- The `@layer base` rule for `select` supplies the font, tracking and the
	     brand focus glow; these utilities restate only what the old
	     `.select-field` class overrode on top of it. -->
	<select
		class="w-full cursor-pointer rounded-sm border border-border bg-surface-2 px-3 py-2 font-sans text-sm text-foreground transition-colors duration-150 ease-out focus:border-subtle-foreground focus:outline-none"
		value={getLocale()}
		onchange={handleLocaleChange}
	>
		{#each locales as loc (loc)}
			<option value={loc}>{loc === 'en' ? 'English' : 'Italiano'}</option>
		{/each}
	</select>
</Section>
