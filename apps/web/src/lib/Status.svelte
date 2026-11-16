<script lang="ts">
	let { state = 'PENDING', small = false, label }: { state?: string; small?: boolean; label?: string } = $props();
</script>

<span class:small class="status {state.toLowerCase()}"
	><span class="status-dot"></span>{label || (state === 'UP'
		? 'Operational'
		: state === 'DOWN'
			? 'Down'
			: state === 'MAINTENANCE'
				? 'Maintenance'
			: state.charAt(0) + state.slice(1).toLowerCase())}</span
>
