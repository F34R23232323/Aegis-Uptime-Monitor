<script lang="ts">
	let { history = [] }: { history: { state: string; created_at?: string; at?: string }[] } = $props();
</script>

<div class="uptime-bars" aria-label="Recent check states, oldest to newest">
	{#if history.length}{#each history.slice(-60) as h}<div
				class="bar {h.state.toLowerCase()}"
				title={`${h.state} · ${new Date(h.created_at || h.at || '').toLocaleString()}`}
			></div>{/each}{:else}{#each Array(40) as _}<div
				class="bar unknown"
				title="No observation"
			></div>{/each}{/if}
</div>
