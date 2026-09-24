<script lang="ts">
	import { Button, Field, Input, Section, Select } from '$lib/ui';
	import { Sparkles, Download } from '@lucide/svelte';
	import { llmListModels, type ApiFormat, type Effort } from '$lib/ai/bridge';
	import { cn } from '$lib/utils';
	import * as m from '$lib/paraglide/messages.js';

	const HINT = 'm-0 font-sans text-xs italic text-subtle-foreground';

	const DEFAULT_BASE: Record<ApiFormat, string> = {
		openai: 'https://api.openai.com/v1',
		anthropic: 'https://api.anthropic.com'
	};

	interface Props {
		apiFormat: ApiFormat;
		baseUrl: string;
		apiKey: string;
		model: string;
		effort: Effort | null;
	}

	let {
		apiFormat = $bindable(),
		baseUrl = $bindable(),
		apiKey = $bindable(),
		model = $bindable(),
		effort = $bindable()
	}: Props = $props();

	let models = $state<string[]>([]);
	let loadingModels = $state(false);
	let modelsError = $state('');

	function clearModels() {
		models = [];
		modelsError = '';
	}

	function handleFormatChange(value: string) {
		const next = value as ApiFormat;
		apiFormat = next;
		baseUrl = DEFAULT_BASE[next];
		clearModels();
	}

	function handleEffortChange(value: string) {
		effort = value === 'default' ? null : (value as Effort);
	}

	async function handleLoadModels() {
		loadingModels = true;
		clearModels();
		try {
			models = await llmListModels({
				api_format: apiFormat,
				base_url: baseUrl.trim(),
				api_key: apiKey.trim(),
				model: model.trim(),
				effort
			});
		} catch (err) {
			modelsError = m.settings_ai_models_failed({ error: String(err) });
		} finally {
			loadingModels = false;
		}
	}
</script>

<Section title={m.settings_ai_title()} icon={Sparkles} collapsible defaultOpen={false}>
	<Field label={m.settings_ai_format()} forId="aiFormat">
		<Select
			id="aiFormat"
			value={apiFormat}
			onchange={handleFormatChange}
			options={[
				{ value: 'openai', label: m.settings_ai_format_openai() },
				{ value: 'anthropic', label: m.settings_ai_format_anthropic() }
			]}
		/>
	</Field>

	<Field label={m.settings_ai_base_url()} forId="aiBaseUrl">
		<Input
			id="aiBaseUrl"
			bind:value={baseUrl}
			placeholder={DEFAULT_BASE[apiFormat]}
			onchange={clearModels}
			mono
		/>
	</Field>

	<Field label={m.settings_ai_api_key()} forId="aiApiKey" hint={m.settings_ai_api_key_hint()}>
		<Input id="aiApiKey" bind:value={apiKey} type="password" placeholder="••••••••" mono />
	</Field>

	<Field label={m.settings_ai_model()} forId="aiModel">
		<Input
			id="aiModel"
			bind:value={model}
			placeholder={m.settings_ai_model_placeholder()}
			list={models.length > 0 ? 'aiModelOptions' : undefined}
			mono
		/>
		{#if models.length > 0}
			<datalist id="aiModelOptions">
				{#each models as id (id)}
					<option value={id}></option>
				{/each}
			</datalist>
		{/if}
	</Field>

	<Field label={m.settings_ai_effort()} forId="aiEffort" hint={m.settings_ai_effort_hint()}>
		<Select
			id="aiEffort"
			value={effort ?? 'default'}
			onchange={handleEffortChange}
			options={[
				{ value: 'default', label: m.settings_ai_effort_default() },
				{ value: 'low', label: m.settings_ai_effort_low() },
				{ value: 'medium', label: m.settings_ai_effort_medium() },
				{ value: 'high', label: m.settings_ai_effort_high() }
			]}
		/>
	</Field>

	<div class="flex flex-wrap items-center gap-2">
		<Button variant="secondary" icon={Download} onclick={handleLoadModels} loading={loadingModels}>
			{loadingModels ? m.settings_ai_loading_models() : m.settings_ai_load_models()}
		</Button>
		{#if models.length > 0}
			<p class={cn(HINT, 'text-positive')}>
				{m.settings_ai_models_loaded({ count: models.length })}
			</p>
		{/if}
	</div>

	{#if modelsError}
		<p class={cn(HINT, 'text-destructive')}>{modelsError}</p>
	{/if}

	<p class={HINT}>{m.settings_ai_privacy()}</p>
</Section>
