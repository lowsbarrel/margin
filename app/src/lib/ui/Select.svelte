<script lang="ts">
	import * as Select from '$lib/components/ui/select/index.js';
	import { cn } from '$lib/utils';

	export interface SelectOption {
		value: string;
		label: string;
	}

	interface Props {
		options: SelectOption[];
		value?: string;
		onchange?: (value: string) => void;
		id?: string;
		disabled?: boolean;
		placeholder?: string;
	}

	let {
		options,
		value = $bindable(''),
		onchange,
		id,
		disabled = false,
		placeholder
	}: Props = $props();

	const TRIGGER =
		'h-8.5 w-full cursor-pointer rounded-sm bg-background py-0 pr-8.5 pl-3 font-sans text-sm text-foreground transition-colors duration-120 ease-out outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 data-[size=default]:h-8.5 dark:bg-background dark:hover:bg-background [&_svg]:size-3.5 [&_svg]:text-subtle-foreground';
	const CONTENT = 'rounded-md border border-border p-1 shadow-(--shadow-lg) ring-0';
	const ITEM =
		'rounded-sm py-1.75 pl-2.5 text-muted-foreground data-highlighted:bg-surface-1 data-highlighted:text-foreground';

	function handleValueChange(next: unknown) {
		value = String(next ?? '');
		onchange?.(value);
	}
</script>

<Select.Root type="single" {value} {disabled} onValueChange={handleValueChange}>
	<Select.Trigger {id} class={cn(TRIGGER)}>
		<Select.Value {placeholder} />
	</Select.Trigger>
	<Select.Content class={CONTENT}>
		{#each options as option (option.value)}
			<Select.Item value={option.value} label={option.label} class={ITEM}
				>{option.label}</Select.Item
			>
		{/each}
	</Select.Content>
</Select.Root>
