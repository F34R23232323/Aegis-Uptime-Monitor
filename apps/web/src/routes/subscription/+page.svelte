<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	let action = $state<{ kind: 'confirm' | 'unsubscribe'; id: string; token: string } | null>(null);
	let message = $state('');
	let busy = $state(false);
	onMount(() => {
		const match = /^#(confirm|unsubscribe)=([0-9a-f-]{36}):([0-9a-f]{64})$/i.exec(location.hash);
		if (match) action = { kind: match[1] as 'confirm' | 'unsubscribe', id: match[2], token: match[3] };
		else message = 'This subscription link is invalid.';
	});
	async function finish() {
		if (!action) return;
		busy = true;
		try {
			const result = await api<{ message: string }>('/public/subscriptions/' + action.kind, 'POST', { id: action.id, token: action.token });
			message = result.message;
			action = null;
			history.replaceState(history.state, '', location.pathname);
		} catch (e) { message = (e as Error).message; }
		finally { busy = false; }
	}
</script>
<svelte:head><title>Status email subscription</title><meta name="robots" content="noindex,nofollow" /></svelte:head>
<main class="auth-page"><section class="auth-card">
	<h1>Status email subscription</h1>
	{#if action}<p>{action.kind === 'confirm' ? 'Confirm that you want published incident updates by email.' : 'Stop receiving published incident updates by email.'}</p>
		<button class="button primary" disabled={busy} onclick={() => void finish()}>{action.kind === 'confirm' ? 'Confirm subscription' : 'Unsubscribe'}</button>
	{/if}
	{#if message}<p role="status">{message}</p>{/if}
</section></main>
