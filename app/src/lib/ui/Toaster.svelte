<script lang="ts">
	import { toast } from '$lib/stores/toast.svelte';
	import { Check, CircleAlert, Info, X } from '@lucide/svelte';
	import { cn } from '$lib/utils';
</script>

{#if toast.items.length > 0}
	<div class="pointer-events-none fixed right-4 bottom-11 z-200 flex flex-col gap-2">
		{#each toast.items as item (item.id)}
			<div
				class="toast-in pointer-events-auto flex max-w-[380px] items-center gap-2.5 rounded-md border border-border bg-background px-3 py-2.5 text-foreground shadow-(--shadow-lg)"
				role="alert"
			>
				<span
					class={cn(
						'flex shrink-0 items-center',
						item.type === 'success' && 'text-(--color-text-positive)',
						item.type === 'error' && 'text-(--color-text-negative)',
						item.type !== 'success' && item.type !== 'error' && 'text-subtle-foreground'
					)}
				>
					{#if item.type === 'success'}
						<Check size={14} />
					{:else if item.type === 'error'}
						<CircleAlert size={14} />
					{:else}
						<Info size={14} />
					{/if}
				</span>
				<span class="min-w-0 flex-1 font-sans text-sm tracking-normal">{item.message}</span>
				{#if item.action}
					<button
						class="shrink-0 cursor-pointer rounded-sm bg-primary px-2.5 py-1 text-xs font-medium text-primary-foreground transition-opacity duration-120 ease-out hover:opacity-[0.86]"
						onclick={() => {
							item.action!.onClick();
							toast.dismiss(item.id);
						}}
					>
						{item.action.label}
					</button>
				{/if}
				<button
					class="flex size-[22px] shrink-0 items-center justify-center rounded-xs bg-transparent p-0 text-subtle-foreground transition-colors duration-120 ease-out hover:bg-muted hover:text-foreground"
					onclick={() => toast.dismiss(item.id)}
				>
					<X size={12} />
				</button>
			</div>
		{/each}
	</div>
{/if}

<style>
	.toast-in {
		animation: toast-in var(--duration-base) var(--ease-out);
	}

	@keyframes toast-in {
		from {
			opacity: 0;
			transform: translateY(6px) scale(0.98);
		}
		to {
			opacity: 1;
			transform: translateY(0) scale(1);
		}
	}
</style>
