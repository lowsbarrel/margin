<script lang="ts">
	import { Button } from '$lib/ui';
	import OnboardingWizard from '$lib/components/onboarding/OnboardingWizard.svelte';
	import WizardFolderStep from './WizardFolderStep.svelte';
	import WizardNameStep from './WizardNameStep.svelte';
	import WizardPassphraseStep from './WizardPassphraseStep.svelte';
	import { CARD_WITHOUT_GAP } from './card-styles';
	import type { LoginState } from './login-state.svelte';
	import * as m from '$lib/paraglide/messages.js';

	const WIZARD_STEPS = ['folder', 'name', 'passphrase'];

	interface Props {
		login: LoginState;
		hasProfiles: boolean;
		onback?: () => void;
	}

	let { login, hasProfiles, onback }: Props = $props();
	let wizardStep = $state(0);
</script>

<div class="{CARD_WITHOUT_GAP} gap-3.5">
	<OnboardingWizard
		stepIds={WIZARD_STEPS}
		bind:step={wizardStep}
		canAdvance={wizardStep > 0 || login.vaultPath.trim() !== ''}
		exitLabel={hasProfiles ? m.login_back_to_vaults() : undefined}
		onExit={hasProfiles ? onback : undefined}
	>
		{#snippet children(id)}
			{#if id === 'folder'}
				<WizardFolderStep vaultPath={login.vaultPath} onpick={login.pickDirectory} />
			{:else if id === 'name'}
				<WizardNameStep bind:value={login.vaultName} />
			{:else}
				<WizardPassphraseStep
					bind:value={login.mnemonic}
					error={login.error}
					showGenerated={login.showGenerated}
					generatedMnemonic={login.generatedMnemonic}
					copied={login.copied}
					ongenerate={login.generatePassphrase}
					oncopy={login.copyPassphrase}
					onuse={login.useGeneratedPassphrase}
				/>
			{/if}
		{/snippet}

		{#snippet finalAction()}
			<Button variant="primary" size="sm" onclick={login.openVault} loading={login.loading}>
				{login.loading ? m.login_opening() : m.login_open_vault()}
			</Button>
		{/snippet}
	</OnboardingWizard>
</div>
