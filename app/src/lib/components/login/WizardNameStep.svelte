<script lang="ts">
	import { CONTROL, FIELD, FIELD_LABEL, STEP_BODY, STEP_TITLE } from './card-styles';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		value?: string;
	}

	let { value = $bindable('') }: Props = $props();
</script>

<div class="flex flex-col gap-1">
	<h2 class={STEP_TITLE}>{m.onboarding_wizard_name_title()}</h2>
	<p class={STEP_BODY}>{m.onboarding_wizard_name_body()}</p>
</div>
<div class="{FIELD} mt-3">
	<label for="vault-name" class={FIELD_LABEL}>{m.login_vault_name_label()}</label>
	<input
		id="vault-name"
		type="text"
		bind:value
		placeholder={m.login_vault_name_placeholder()}
		class="{CONTROL} font-sans"
	/>
</div>
