<script lang="ts">
	import { Button, Field, Input, Section } from '$lib/ui';
	import { Sparkles, Download } from '@lucide/svelte';
	import { llmListModels, type ApiFormat, type Effort } from '$lib/ai/bridge';
	import { cn } from '$lib/utils';
	import * as m from '$lib/paraglide/messages.js';

	/* One hint line and the load-models result share this; only the colour differs. */
	const HINT = 'm-0 font-sans text-xs italic text-subtle-foreground';

	/** What the two formats' own hosts are, so switching format is one click. */
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

	/** A list loaded from one endpoint must not be offered against another. */
	function clearModels() {
		models = [];
		modelsError = '';
	}

	function handleFormatChange(event: Event) {
		const next = (event.target as HTMLSelectElement).value as ApiFormat;
		apiFormat = next;
		baseUrl = DEFAULT_BASE[next];
		clearModels();
	}

	/** The empty option is the absence of an effort, not a fourth level. */
	function handleEffortChange(event: Event) {
		const value = (event.target as HTMLSelectElement).value;
		effort = value === '' ? null : (value as Effort);
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
		<!-- The `@layer base` rule for `select` supplies the font, tracking and the
		     brand focus glow; these utilities restate only what the old
		     `.select-field` class overrode on top of it. -->
		<select
			class="w-full cursor-pointer rounded-sm border border-border bg-surface-2 px-3 py-2 font-sans text-sm text-foreground transition-colors duration-150 ease-out focus:border-subtle-foreground focus:outline-none"
			id="aiFormat"
			value={apiFormat}
			onchange={handleFormatChange}
		>
			<option value="openai">{m.settings_ai_format_openai()}</option>
			<option value="anthropic">{m.settings_ai_format_anthropic()}</option>
		</select>
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
		<!-- A datalist rather than a select: hosted routers list hundreds of models,
		     so the field stays typeable and filters the loaded list as you type. -->
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
		<select
			class="w-full cursor-pointer rounded-sm border border-border bg-surface-2 px-3 py-2 font-sans text-sm text-foreground transition-colors duration-150 ease-out focus:border-subtle-foreground focus:outline-none"
			id="aiEffort"
			value={effort ?? ''}
			onchange={handleEffortChange}
		>
			<option value="">{m.settings_ai_effort_default()}</option>
			<option value="low">{m.settings_ai_effort_low()}</option>
			<option value="medium">{m.settings_ai_effort_medium()}</option>
			<option value="high">{m.settings_ai_effort_high()}</option>
		</select>
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
