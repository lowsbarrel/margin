<script lang="ts">
	import { Button, GlassModal } from '$lib/ui';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		title: string;
		body: string;
		confirmLabel: string;
		busy?: boolean;
		onconfirm: () => void;
		oncancel: () => void;
	}

	let { title, body, confirmLabel, busy = false, onconfirm, oncancel }: Props = $props();
</script>

<GlassModal {title} onclose={oncancel} width="400px">
	<p class="text-sm text-muted-foreground">{body}</p>
	<div class="mt-1 flex justify-end gap-2">
		<Button size="sm" variant="ghost" onclick={oncancel}>{m.trash_cancel()}</Button>
		<Button size="sm" variant="danger" loading={busy} onclick={onconfirm}>{confirmLabel}</Button>
	</div>
</GlassModal>
