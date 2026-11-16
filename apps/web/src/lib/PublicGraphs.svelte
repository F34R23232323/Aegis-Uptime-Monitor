<script lang="ts">
	import { Activity, ChartNoAxesCombined } from '@lucide/svelte';
	import type { PublicPage } from './public';
	import type { StatusMetricsSettings } from './api';

	let { summary, settings }: { summary: PublicPage['metrics_summary']; settings: StatusMetricsSettings } =
		$props();
	let uptime = $derived.by(() =>
		settings.uptime_days.map((days) => {
			return { days, value: summary?.uptime.find((item) => item.days === days)?.uptime ?? null };
		})
	);
	let latency = $derived.by(() => {
		return (summary?.latency || []).map((point) => ({
			at: new Date(point.at).getTime(),
			ms: point.latency_ms
		}));
	});
	let latencyMax = $derived(Math.max(1, ...latency.map((point) => point.ms)));
	let line = $derived(
		latency
			.map(
				(point) =>
					`${latency.length === 1 ? 0 : ((point.at - latency[0].at) * 100) / (latency[latency.length - 1].at - latency[0].at || 1)},${100 - (point.ms / latencyMax) * 85}`
			)
			.join(' ')
	);
	let latencyAverage = $derived(
		latency.length ? latency.reduce((sum, point) => sum + point.ms, 0) / latency.length : null
	);
	const graphDate = (at: number) =>
		new Intl.DateTimeFormat('en-GB', {
			timeZone: 'UTC',
			day: '2-digit',
			month: 'short',
			year: 'numeric'
		}).format(at);
</script>

{#if settings.show_uptime || settings.show_latency}
	<section class="public-graphs" aria-label="Measured performance graphs">
		<div class="public-graphs-heading">
			<div>
				<span class="graph-eyebrow">MEASURED PERFORMANCE</span>
				<h2>Reliability over time</h2>
			</div>
			<span>Observed data only</span>
		</div>
		<div class="public-graph-grid">
			{#if settings.show_uptime}<article class="public-graph-card">
					<div class="graph-card-heading">
						<ChartNoAxesCombined size={17} /><strong>Uptime by period</strong>
					</div>
					<p>Time-weighted availability across included checks.</p>
					<div
						class="uptime-chart"
						role="img"
						aria-label={'Uptime: ' +
							uptime
								.map((p) => `${p.days} days ${p.value == null ? 'unavailable' : p.value.toFixed(2) + '%'}`)
								.join(', ')}
					>
						{#each uptime as point}<div class="uptime-column">
								<span class="uptime-value">{point.value == null ? '—' : `${point.value.toFixed(2)}%`}</span>
								<div class="uptime-track">
									<span style={`height:${point.value == null ? 0 : Math.max(2, point.value)}%`}></span>
								</div>
								<small>{point.days === 1 ? '24h' : `${point.days}d`}</small>
							</div>{/each}
					</div>
				</article>{/if}
			{#if settings.show_latency}<article class="public-graph-card">
					<div class="graph-card-heading"><Activity size={17} /><strong>Response time</strong></div>
					<p>Recent successful probe durations, averaged into 15-minute intervals.</p>
					{#if latency.length}<div class="latency-stats">
							<strong>{latencyAverage?.toFixed(0)} <small>ms avg</small></strong><span
								>{latencyMax.toFixed(0)} ms peak</span
							>
						</div>
						<div
							class="latency-chart"
							role="img"
							aria-label={`Response time from ${graphDate(latency[0].at)} to ${graphDate(latency[latency.length - 1].at)}, average ${latencyAverage?.toFixed(0)} milliseconds`}
						>
							<svg viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true"
								><line x1="0" y1="99" x2="100" y2="99" stroke="currentColor" opacity=".2" /><polyline
									points={line}
									fill="none"
									stroke="var(--accent)"
									stroke-width="2"
									vector-effect="non-scaling-stroke"
									stroke-linecap="round"
									stroke-linejoin="round"
								/></svg
							>
						</div>
						<div class="graph-axis">
							<span>{graphDate(latency[0].at)}</span><span>{graphDate(latency[latency.length - 1].at)}</span>
						</div>{:else}<div class="graph-empty">
							Response-time samples will appear after successful checks.
						</div>{/if}
				</article>{/if}
		</div>
	</section>
{/if}

<style>
	.public-graphs {
		margin: 28px 0 34px;
	}
	.public-graphs-heading {
		display: flex;
		justify-content: space-between;
		align-items: end;
		gap: 15px;
		margin-bottom: 14px;
	}
	.public-graphs-heading h2 {
		font-size: 20px;
		margin-top: 4px;
	}
	.public-graphs-heading > span,
	.graph-eyebrow {
		font-size: 10px;
		letter-spacing: 1.2px;
		color: var(--muted);
		text-transform: uppercase;
	}
	.public-graph-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 340px), 1fr));
		gap: 14px;
	}
	.public-graph-card {
		min-width: 0;
		border: 1px solid var(--border);
		border-radius: 14px;
		padding: 19px 21px;
		background: var(--panel);
	}
	.graph-card-heading {
		display: flex;
		align-items: center;
		gap: 9px;
		color: var(--accent);
	}
	.graph-card-heading strong {
		color: inherit;
		font-size: 14px;
	}
	.public-graph-card p {
		color: var(--muted);
		font-size: 11px;
		margin: 5px 0 20px;
	}
	.uptime-chart {
		height: 170px;
		display: flex;
		gap: 15px;
		justify-content: space-around;
	}
	.uptime-column {
		min-width: 42px;
		height: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 7px;
		flex: 1;
	}
	.uptime-value {
		font-size: 11px;
		font-variant-numeric: tabular-nums;
	}
	.uptime-track {
		width: 100%;
		max-width: 55px;
		flex: 1;
		border-radius: 6px;
		background: color-mix(in srgb, var(--accent) 10%, transparent);
		display: flex;
		align-items: end;
		overflow: hidden;
	}
	.uptime-track span {
		width: 100%;
		display: block;
		background: var(--accent);
		border-radius: 5px 5px 0 0;
	}
	.uptime-column small {
		color: var(--muted);
	}
	.latency-stats {
		display: flex;
		justify-content: space-between;
		align-items: end;
		margin-bottom: 8px;
	}
	.latency-stats strong {
		font-size: 22px;
		letter-spacing: -0.6px;
	}
	.latency-stats strong small,
	.latency-stats span {
		font-size: 11px;
		color: var(--muted);
		letter-spacing: 0;
	}
	.latency-chart {
		height: 115px;
		color: var(--border);
	}
	.latency-chart svg {
		width: 100%;
		height: 100%;
		overflow: visible;
	}
	.graph-axis {
		display: flex;
		justify-content: space-between;
		color: var(--muted);
		font-size: 10px;
		margin-top: 6px;
	}
	.graph-empty {
		display: grid;
		place-items: center;
		min-height: 140px;
		color: var(--muted);
		font-size: 12px;
		text-align: center;
	}
</style>
