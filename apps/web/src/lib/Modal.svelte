<script lang="ts">
	import { Dialog } from 'bits-ui';
	import { X } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	let {
		open = $bindable(false),
		title,
		description = '',
		children
	}: { open?: boolean; title: string; description?: string; children: Snippet } = $props();
</script>

<Dialog.Root bind:open
	><Dialog.Portal
		><Dialog.Overlay class="modal-overlay" /><Dialog.Content class="modal-panel"
			><div class="modal-heading">
				<div>
					<Dialog.Title class="modal-title">{title}</Dialog.Title><Dialog.Description class="muted"
						>{description}</Dialog.Description
					>
				</div>
				<Dialog.Close class="icon-button" aria-label="Close"><X size={20} /></Dialog.Close>
			</div>
			{@render children()}</Dialog.Content
		></Dialog.Portal
	></Dialog.Root
>
