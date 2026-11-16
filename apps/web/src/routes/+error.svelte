<script lang="ts">
	import { page } from '$app/state';
	import { ShieldCheck } from '@lucide/svelte';
</script>

<main class="boot">
	<ShieldCheck size={36} />
	<h1>{page.status}</h1>
	<p>{page.error?.message || 'Something went wrong.'}</p>
	<a class="button primary" href="/">Back to overview</a>
</main>
