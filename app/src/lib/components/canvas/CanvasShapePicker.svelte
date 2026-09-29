<script lang="ts">
	import type { ShapeKind } from '$lib/canvas/types';
	import { shapeKinds } from '$lib/canvas/types';
	import { shapeIcons, shapeLabels } from './shape-ui';
	import { toolButtonClass } from './control-classes';

	interface Props {
		shape: ShapeKind;
		onselect: (shape: ShapeKind) => void;
	}

	let { shape, onselect }: Props = $props();
</script>

<div class="flex items-center gap-0.5 p-0.5">
	{#each shapeKinds as kind (kind)}
		{@const Icon = shapeIcons[kind]}
		<button
			class={toolButtonClass(shape === kind)}
			onclick={() => onselect(kind)}
			title={shapeLabels[kind]()}
			aria-pressed={shape === kind}
		>
			<Icon size={16} />
		</button>
	{/each}
</div>
