<script lang="ts">
	import type { HostMetricSample } from './api';
	let { samples }: { samples: HostMetricSample[] } = $props();
	const series = [
		{ key: 'cpu_percent', label: 'CPU', color: '#a18aff' },
		{ key: 'memory_percent', label: 'Memory', color: '#55d5bd' },
		{ key: 'disk_percent', label: 'Disk', color: '#f0b56d' }
	] as const;
	function points(key: (typeof series)[number]['key']) {
		return samples
			.map(
				(sample, index) =>
					`${samples.length < 2 ? 0 : (index * 1000) / (samples.length - 1)},${180 - Math.max(0, Math.min(100, sample[key] ?? 0)) * 1.6}`
			)
			.join(' ');
	}
</script>

<div class="host-history" role="img" aria-label="Historical host CPU, memory and disk use as percentages">
	{#if samples.length}
		<div class="host-history-legend">
			{#each series as item}<span style={`--series-color:${item.color}`}>{item.label}</span>{/each}
		</div>
		<svg viewBox="0 0 1000 190" preserveAspectRatio="none" aria-hidden="true">
			{#each [20, 100, 180] as y}<line
					x1="0"
					x2="1000"
					y1={y}
					y2={y}
					stroke="var(--border)"
					stroke-dasharray="4 5"
				/>{/each}
			{#each series as item}<polyline
					points={points(item.key)}
					fill="none"
					stroke={item.color}
					stroke-width="2"
					vector-effect="non-scaling-stroke"
				/>{/each}
		</svg>
		<div class="chart-labels">
			<span>{new Date(samples[0].bucket).toLocaleString()}</span><span
				>{new Date(samples[samples.length - 1].bucket).toLocaleString()}</span
			>
		</div>
	{:else}<p class="muted small-text">History appears after the agent sends its first metric sample.</p>{/if}
</div>
