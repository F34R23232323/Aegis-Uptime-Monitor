<script lang="ts">
	import type { Check } from './api';
	let { checks = [], compact = false }: { checks: Check[]; compact?: boolean } = $props();
	let hover = $state<number | null>(null);
	let max = $derived(Math.max(10, ...checks.map((c) => c.latency_ms)) * 1.2);
	let points = $derived(
		checks
			.map(
				(c, i) =>
					`${checks.length < 2 ? 0 : (i / (checks.length - 1)) * 1000},${180 - (c.latency_ms / max) * 155}`
			)
			.join(' ')
	);
</script>

<div class:compact class="chart" role="img" aria-label="Measured response latency over recent checks">
	{#if checks.length > 0}
		{#if !compact}<div class="chart-scale">
				<span>{Math.round(max)} ms</span><span>{Math.round(max / 2)} ms</span><span>0 ms</span>
			</div>{/if}
		<svg viewBox="0 0 1000 190" preserveAspectRatio="none" aria-hidden="true"
			><defs
				><linearGradient id="chart-fill" x1="0" y1="0" x2="0" y2="1"
					><stop offset="0%" stop-color="var(--accent)" stop-opacity=".25" /><stop
						offset="100%"
						stop-color="var(--accent)"
						stop-opacity="0"
					/></linearGradient
				></defs
			>{#if !compact}{#each [25, 95, 180] as y}<line
						x1="0"
						x2="1000"
						y1={y}
						y2={y}
						stroke="var(--border)"
						stroke-dasharray="3 5"
					/>{/each}{/if}<polygon points={`0,190 ${points} 1000,190`} fill="url(#chart-fill)" /><polyline
				{points}
				fill="none"
				stroke="var(--accent)"
				stroke-width={compact ? 3 : 2}
				vector-effect="non-scaling-stroke"
				stroke-linejoin="round"
			/>
		</svg>
		{#if !compact}<div
				class="chart-hitbox"
				onpointermove={(e) => {
					const r = e.currentTarget.getBoundingClientRect();
					hover = Math.min(
						checks.length - 1,
						Math.max(0, Math.round(((e.clientX - r.left) / r.width) * (checks.length - 1)))
					);
				}}
				onpointerleave={() => (hover = null)}
				role="presentation"
			></div>
			{#if hover !== null}<div
					class="chart-tooltip"
					style={`left:${Math.min(80, (hover / Math.max(1, checks.length - 1)) * 100)}%`}
				>
					{checks[hover].latency_ms.toFixed(1)} ms<br /><small
						>{new Date(checks[hover].created_at).toLocaleTimeString()}</small
					>
				</div>{/if}
			<div class="chart-labels">
				<span>{new Date(checks[0].created_at).toLocaleTimeString()}</span><span
					>{new Date(checks[checks.length - 1].created_at).toLocaleTimeString()}</span
				>
			</div>{/if}
	{:else}<div class="chart-empty">Response data appears after the first check.</div>{/if}
</div>
