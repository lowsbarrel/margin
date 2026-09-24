<script lang="ts">
	import { Select, Section } from '$lib/ui';
	import { Globe } from '@lucide/svelte';
	import { getLocale, setLocale, locales, type Locale } from '$lib/paraglide/runtime.js';
	import * as m from '$lib/paraglide/messages.js';

	function handleLocaleChange(value: string) {
		if ((locales as readonly string[]).includes(value)) {
			setLocale(value as Locale);
		}
	}
</script>

<Section title={m.settings_language()} icon={Globe} collapsible defaultOpen={false}>
	<Select
		id="locale"
		value={getLocale()}
		onchange={handleLocaleChange}
		options={locales.map((loc) => ({ value: loc, label: loc === 'en' ? 'English' : 'Italiano' }))}
	/>
</Section>
