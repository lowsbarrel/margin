<script lang="ts">
	import { onMount } from 'svelte';
	import { theme } from '$lib/stores/theme.svelte';
	import { IconButton } from '$lib/ui';
	import OnboardingIntro from '$lib/components/onboarding/OnboardingIntro.svelte';
	import LoginBrand from './login/LoginBrand.svelte';
	import VaultPicker from './login/VaultPicker.svelte';
	import VaultWizard from './login/VaultWizard.svelte';
	import { createLoginState } from './login/login-state.svelte';
	import { Sun, Moon } from '@lucide/svelte';
	import type { VaultProfile } from '$lib/session/bridge';
	import * as m from '$lib/paraglide/messages.js';

	type Screen = 'intro' | 'vaults' | 'wizard';

	const login = createLoginState();
	let screen = $state<Screen>('vaults');

	onMount(async () => {
		await login.load();
		if (login.profiles.length === 0) screen = 'intro';
	});

	async function removeProfile(profile: VaultProfile) {
		const removed = await login.removeProfile(profile);
		if (removed && login.profiles.length === 0) screen = 'intro';
	}

	function startNewVault() {
		login.resetWizard();
		screen = 'wizard';
	}

	function replayIntro() {
		screen = 'intro';
	}

	function goBack() {
		login.resetWizard();
		screen = 'vaults';
	}

	function finishIntro() {
		if (login.profiles.length > 0) goBack();
		else screen = 'wizard';
	}
</script>

<div class="relative h-screen overflow-y-auto p-8">
	<div
		class="relative mx-auto flex min-h-[calc(100vh-4rem)] w-[min(100%,380px)] flex-col items-stretch justify-center [@media(max-height:760px)]:min-h-0 [@media(max-height:760px)]:justify-start [@media(max-height:760px)]:pt-10 [@media(max-height:760px)]:pb-4"
	>
		{#if login.autoLogging}
			<div class="flex flex-col items-center gap-6">
				<LoginBrand />
				<span class="text-sm text-subtle-foreground italic">{m.login_auto_opening()}</span>
			</div>
		{:else}
			<div class="fixed top-4 right-4">
				<IconButton
					icon={theme.current === 'dark' ? Sun : Moon}
					onclick={() => theme.toggle()}
					title={m.statusbar_toggle_theme()}
				/>
			</div>

			{#if screen === 'intro'}
				<OnboardingIntro onDone={finishIntro} />
			{:else}
				<LoginBrand tagline />

				{#if screen === 'vaults'}
					<VaultPicker
						profiles={login.profiles}
						error={login.error}
						onselect={login.selectProfile}
						onremove={removeProfile}
						onadd={startNewVault}
						onreplay={replayIntro}
					/>
				{:else}
					<VaultWizard {login} hasProfiles={login.profiles.length > 0} onback={goBack} />
				{/if}
			{/if}
		{/if}
	</div>
</div>
