<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { page } from '$app/state';
	import { replaceState } from '$app/navigation';
	import {
		ShieldCheck,
		CheckCheck,
		TriangleAlert,
		RefreshCw,
		ArrowUpRight,
		Clock,
		Wrench,
		Rss,
		MessageSquare,
		Layers,
		Activity,
		ExternalLink
	} from '@lucide/svelte';
	import { api, percent, defaultMetricsSettings } from '#lib/api.ts';
	import type { PublicPage } from '#lib/public.ts';
	import { statusLabels } from '#lib/public.ts';
	import Status from '#lib/Status.svelte';
	import Bars from '#lib/Bars.svelte';
	import PublicGraphs from '#lib/PublicGraphs.svelte';
	let { data: initial }: { data: { status: PublicPage } } = $props();
	let data = $state<PublicPage>(untrack(() => initial.status));
	let error = $state('');
	// Reused route components must adopt the newly server-projected page on navigation.
	$effect(() => {
		const incoming = initial.status;
		untrack(() => {
			data = incoming;
			updated = incoming.generated_at;
			error = '';
			const hash = typeof location === 'undefined' ? '' : location.hash.slice(1);
			activeTab = incoming.tabs.find((t) => t.id === hash)?.id || incoming.tabs[0]?.id || 'overview';
		});
	});
	let activeTab = $state(untrack(() => data.tabs[0]?.id || 'overview'));
	let period = $state(1);
	let systemLight = $state(false);
	let updated = $state(untrack(() => data.generated_at));
	let timezone = $state('UTC');
	let language = $state('en-GB');
	let selected = $derived(data.tabs.find((t) => t.id === activeTab) || data.tabs[0]);
	let activeIncidents = $derived(data.incidents.filter((i) => i.phase !== 'resolved'));
	let activeMaintenance = $derived(
		data.maintenance.filter(
			(w) => new Date(w.starts_at).getTime() <= Date.now() && new Date(w.ends_at).getTime() > Date.now()
		)
	);
	let groups = $derived.by(() => {
		const result = new Map<string, typeof data.resources>();
		for (const r of data.resources) {
			const key = r.group_name || 'Services';
			result.set(key, [...(result.get(key) || []), r]);
		}
		return [...result.entries()];
	});
	let light = $derived(data.theme === 'light' || (data.theme === 'auto' && systemLight));
	let metricsSettings = $derived(data.metrics_settings || defaultMetricsSettings);
	let issue = $derived(
		['major_outage', 'partial_outage', 'degraded'].includes(data.overall_state) || activeIncidents.length > 0
	);
	let heroTitle = $derived(
		activeIncidents.length > 0 && data.overall_state === 'operational'
			? 'An incident is being tracked'
			: data.overall_state === 'operational'
				? 'All systems operational'
				: data.overall_state === 'maintenance'
					? 'Scheduled maintenance in progress'
					: data.overall_state === 'unknown'
						? 'Monitoring in progress'
						: 'Some services are experiencing issues'
	);
	function time(value: string | null) {
		return value
			? new Intl.DateTimeFormat(language, {
					dateStyle: 'medium',
					timeStyle: 'short',
					timeZone: timezone
				}).format(new Date(value))
			: '—';
	}
	function selectTab(id: string) {
		activeTab = id;
		replaceState('#' + id, {});
	}
	async function load() {
		try {
			data = await api<PublicPage>('/public/status/' + encodeURIComponent(page.params.slug || ''));
			updated = data.generated_at;
			error = '';
			if (!data.tabs.some((t) => t.id === activeTab)) activeTab = data.tabs[0]?.id || 'overview';
		} catch (e) {
			error = (e as Error).message;
		}
	}
	onMount(() => {
		timezone = Intl.DateTimeFormat().resolvedOptions().timeZone;
		language = navigator.language;
		const syncHash = () => {
			const hash = location.hash.slice(1);
			if (data.tabs.some((t) => t.id === hash)) activeTab = hash;
		};
		syncHash();
		window.addEventListener('hashchange', syncHash);
		const mq = matchMedia('(prefers-color-scheme: light)');
		systemLight = mq.matches;
		const listener = () => (systemLight = mq.matches);
		mq.addEventListener('change', listener);
		let refreshing = false;
		const timer = setInterval(() => {
			if (!refreshing) {
				refreshing = true;
				void load().finally(() => (refreshing = false));
			}
		}, 30000);
		return () => {
			clearInterval(timer);
			window.removeEventListener('hashchange', syncHash);
			mq.removeEventListener('change', listener);
		};
	});
</script>

<svelte:head
	><title>{data.title} · Service status</title><meta
		name="description"
		content={data.description || 'Live service availability and incident history.'}
	/><meta property="og:title" content={data.title} /><meta
		property="og:description"
		content={data.description || 'Live service health and incident communications.'}
	/><meta name="theme-color" content={light ? '#f6f7fb' : '#0b0c11'} /><link
		rel="canonical"
		href={page.url.origin + page.url.pathname}
	/><link
		rel="alternate"
		type="application/rss+xml"
		title={data.title + ' incidents'}
		href={'/api/v1/public/status/' + data.slug + '/feed'}
	/></svelte:head
>
<div
	class="public-site"
	class:light
	class:provider={data.layout === 'provider'}
	style={`--accent:${data.accent}`}
>
	<main class="public-page public-page-v2">
		<div class="public-atmosphere"></div>
		<header class="public-header">
			<a class="brand" href={page.url.pathname}
				>{#if data.logo_url}<img
						class="public-logo"
						src={data.logo_url}
						alt=""
						referrerpolicy="no-referrer"
					/>{:else}<span class="brand-mark"><ShieldCheck size={23} /></span>{/if}{data.title}</a
			>
			<div class="public-header-actions">
				{#if data.support_url}<a href={data.support_url} target="_blank" rel="noreferrer" class="subtle-link"
						>Contact support <ExternalLink size={12} /></a
					>{/if}<a
					href={'/api/v1/public/status/' + data.slug + '/feed'}
					class="icon-button"
					aria-label="RSS incident feed"><Rss size={16} /></a
				>
			</div>
		</header>
		{#if data.announcement}<aside class="public-announcement">
				<MessageSquare size={17} />
				<p>{data.announcement}</p>
			</aside>{/if}{#if error}<div class="error public-refresh-error" role="alert">
				Live refresh interrupted. Showing the last verified data from {time(updated)}. {error}<button
					class="button small"
					onclick={() => void load()}>Retry</button
				>
			</div>{/if}
		<section class="public-hero">
			<div class="eyebrow">
				{data.layout === 'provider' ? 'PROVIDER COMPONENT STATUS' : 'A CLEARER PICTURE'}
			</div>
			<h1>{data.title}</h1>
			<p>
				{data.description ||
					(data.layout === 'provider'
						? 'Independent observations of published provider components.'
						: 'A transparent view of how our services are doing.')}
			</p>
			<div class:has-outage={issue} class="public-health">
				{#if issue}<TriangleAlert size={26} />{:else if data.overall_state === 'maintenance'}<Wrench
						size={26}
					/>{:else}<CheckCheck size={26} />{/if}
				<div>
					<h2>{heroTitle}</h2>
					<p>
						{activeIncidents.length
							? `${activeIncidents.length} active incident${activeIncidents.length === 1 ? '' : 's'} · Latest updates below.`
							: data.overall_state === 'operational'
								? 'Current observations indicate normal operation.'
								: data.overall_state === 'unknown'
									? 'Some services are waiting for sufficient recent observations.'
									: 'See the affected resources and maintenance notices below.'}
					</p>
				</div>
				<span class="live-dot"></span>
			</div>
		</section>
		<nav class="public-navigation" aria-label="Public status sections">
			{#each data.tabs as tab}<a
					href={'#' + tab.id}
					class:active={selected?.id === tab.id}
					aria-current={selected?.id === tab.id ? 'page' : undefined}
					onclick={(event) => {
						event.preventDefault();
						selectTab(tab.id);
					}}
					>{tab.label}{#if tab.kind === 'incidents' && activeIncidents.length}<span
							>{activeIncidents.length}</span
						>{/if}</a
				>{/each}
		</nav>
		{#if selected?.kind === 'overview'}
			{#if activeIncidents.length}<section class="public-incident-banner">
					<div>
						<TriangleAlert size={18} />
						<div>
							<strong>{activeIncidents[0].title}</strong>
							<p>{activeIncidents[0].message}</p>
						</div>
					</div>
					{#if data.tabs.some((t) => t.kind === 'incidents')}<button
							class="text-button"
							onclick={() => selectTab(data.tabs.find((t) => t.kind === 'incidents')!.id)}
							>Read incident updates →</button
						>{/if}
				</section>{/if}
			{#if activeMaintenance.length}<div class="public-maintenance-strip">
					<Wrench size={16} /><span
						>{activeMaintenance[0].title} · Until {time(activeMaintenance[0].ends_at)}</span
					>
				</div>{/if}
			{#if data.layout === 'provider'}<div class="provider-overview-summary">
					<div><strong>{data.resources.length}</strong><span>Components monitored</span></div>
					<div>
						<strong>{data.resources.filter((r) => r.state === 'operational').length}</strong><span
							>Operational</span
						>
					</div>
					<div>
						<strong
							>{data.resources.filter((r) => ['degraded', 'partial_outage', 'major_outage'].includes(r.state))
								.length}</strong
						><span>With reported issues</span>
					</div>
				</div>{/if}
			{#if metricsSettings.show_on_overview}<PublicGraphs
					summary={data.metrics_summary}
					settings={metricsSettings}
				/>{/if}
			{#each groups as [group, resources]}<section class="public-resource-group">
					<div class="section-heading">
						<h2>{group}</h2>
						<span class="muted small-text"
							>{resources.length} resource{resources.length === 1 ? '' : 's'}</span
						>
					</div>
					<div class="panel" class:provider-component-grid={data.layout === 'provider'}>
						{#each resources as r}<article class="public-resource">
								<div class="row-between">
									<div>
										<h3>{r.name}</h3>
										{#if r.description}<p class="muted">{r.description}</p>{/if}
									</div>
									<span class="resource-badge {r.state}"><span></span>{statusLabels[r.state]}</span>
								</div>
								{#if r.status_source === 'manual'}<div class="public-override">
										<span class="tag">Manual status</span>
										<p>{r.override_reason}</p>
										{#if r.override_until}<small>Returns to automatic status {time(r.override_until)}</small
											>{/if}
									</div>{/if}
								<div class="resource-availability">
									<span
										>{percent(r.uptime.find((u) => u.days === 1)?.uptime)}
										<small>24h linked-monitor availability</small></span
									><span class="muted small-text"
										>{data.layout === 'provider' && metricsSettings.show_latency && r.latency?.length
											? `${Math.round(r.latency[r.latency.length - 1].latency_ms)} ms latest response`
											: `${Math.round((r.uptime.find((u) => u.days === 1)?.covered_seconds || 0) / 60)} observed monitor-minutes`}</span
									>
								</div>
							</article>{/each}
					</div>
				</section>{/each}
			{#if data.monitors.length}<div class="section-heading">
					<h2>{data.resources.length ? 'Monitored endpoints' : 'Service availability'}</h2>
					<div class="filter-tabs public-tabs">
						{#each [[1, '24h'], [7, '7d'], [30, '30d']] as [days, label]}<button
								class:chosen={period === days}
								onclick={() => (period = Number(days))}>{label}</button
							>{/each}
					</div>
				</div>
				<section class="panel public-services">
					{#each data.monitors as m}<article class="public-service">
							<div class="row-between">
								<h3>{m.name}</h3>
								<Status state={m.state} small />
							</div>
							<Bars history={m.history} />
							<div class="row-between small-text muted">
								<span>Latest {m.history.length} observations</span><span
									>{percent((period === 1 ? m.uptime_24h : period === 7 ? m.uptime_7d : m.uptime_30d).uptime)} observed
									uptime</span
								>
							</div>
						</article>{/each}
				</section>{/if}
			{#if !data.resources.length && !data.monitors.length}<section class="panel empty-state">
					<Layers size={30} />
					<h3>Getting ready to keep you informed.</h3>
					<p>No public resources have been selected yet.</p>
				</section>{/if}
		{:else if selected?.kind === 'incidents'}<section class="public-history">
				<div class="section-heading">
					<h2>Incident history</h2>
					<a href={'/api/v1/public/status/' + data.slug + '/feed'} class="subtle-link"
						><Rss size={13} /> Subscribe with RSS</a
					>
				</div>
				{#each data.incidents as i}<article class="public-incident-card">
						<div class="row-between">
							<span class="tag severity-{i.severity}">{i.severity} incident</span><span
								class="incident-phase {i.phase}">{i.phase}</span
							>
						</div>
						<h3>{i.title}</h3>
						<p class="public-message">{i.message}</p>
						<div class="incident-times">
							<span>Started {time(i.started_at)}</span>{#if i.ended_at}<span>Resolved {time(i.ended_at)}</span
								>{/if}
						</div>
						{#if i.updates.length}<div class="public-timeline">
								{#each i.updates as update}<div>
										<span class="timeline-dot"></span>
										<div class="row-between">
											<strong>{update.phase}</strong><small>{time(update.created_at)}</small>
										</div>
										<p>{update.message}</p>
									</div>{/each}
							</div>{/if}
					</article>{:else}<div class="public-no-incidents">
						<CheckCheck size={20} />
						<p>No published incidents for these services.</p>
					</div>{/each}
				{#if data.monitors.some((m) => m.incidents.length)}<h3 class="observed-events-heading">
						Observed monitoring interruptions
					</h3>
					<p class="muted small-text">
						Automatically recorded interruption times. Private failure details are not published.
					</p>
					{#each data.monitors
						.flatMap((m) => m.incidents.map((i) => ({ ...i, name: m.name })))
						.sort((a, b) => b.started_at.localeCompare(a.started_at))
						.slice(0, 30) as i}<div class="public-incident">
							<span class="event-icon" class:resolved={!!i.ended_at}
								>{#if i.ended_at}<CheckCheck size={16} />{:else}<TriangleAlert size={16} />{/if}</span
							>
							<div>
								<h3>{i.name} · {i.ended_at ? 'Resolved' : 'Investigating'}</h3>
								<p>{time(i.started_at)}{i.ended_at ? ' — ' + time(i.ended_at) : ''}</p>
							</div>
						</div>{/each}{/if}
			</section>
		{:else if selected?.kind === 'maintenance'}<section class="public-history">
				<h2>Planned maintenance</h2>
				{#each data.maintenance as w}<article class="public-maintenance-card">
						<div class="row-between">
							<span class="kind-icon"><Wrench size={18} /></span><span class="tag"
								>{new Date(w.ends_at).getTime() < Date.now()
									? 'Completed'
									: new Date(w.starts_at).getTime() <= Date.now()
										? 'In progress'
										: 'Scheduled'}</span
							>
						</div>
						<h3>{w.title}</h3>
						<p class="public-message">{w.message}</p>
						<div class="maintenance-dates">
							<div><span>Starts</span><strong>{time(w.starts_at)}</strong></div>
							<div><span>Ends</span><strong>{time(w.ends_at)}</strong></div>
						</div>
					</article>{:else}<div class="public-no-incidents">
						<Wrench size={20} />
						<p>No published maintenance windows.</p>
					</div>{/each}
			</section>
		{:else if selected?.kind === 'metrics'}<section class="public-history">
				<div class="section-heading">
					<h2>Measured availability</h2>
					<Activity size={18} />
				</div>
				<div class="notice metrics-definition">
					UP / (UP + DOWN), weighted by observed monitor duration. Pending, paused, maintenance, stale and
					missing periods are excluded. Manual presentation overrides do not affect measurements.
				</div>
				<PublicGraphs summary={data.metrics_summary} settings={metricsSettings} />
				<section class="panel">
					<div class="table-scroll">
						<table>
							<thead
								><tr
									><th>Resource / monitor</th><th>24h</th><th>7 days</th><th>30 days</th><th
										>Observed coverage · 24h</th
									></tr
								></thead
							><tbody
								>{#each data.resources as r}<tr
										><td><strong>{r.name}</strong><small class="muted">Linked-monitor weighted</small></td
										>{#each [1, 7, 30] as days}<td
												>{percent(r.uptime.find((u) => u.days === days)?.uptime)}</td
											>{/each}<td
											>{((r.uptime.find((u) => u.days === 1)?.covered_seconds || 0) / 3600).toFixed(2)} monitor-hours</td
										></tr
									>{/each}{#each data.monitors as m}<tr
										><td>{m.name}<small class="muted">Direct monitor</small></td><td
											>{percent(m.uptime_24h.uptime)}</td
										><td>{percent(m.uptime_7d.uptime)}</td><td>{percent(m.uptime_30d.uptime)}</td><td
											>{(m.uptime_24h.covered_seconds / 3600).toFixed(2)} hours</td
										></tr
									>{/each}{#if !data.resources.length && !data.monitors.length}<tr
										><td colspan="5" class="empty-cell">No public observations available.</td></tr
									>{/if}</tbody
							>
						</table>
					</div>
				</section>
			</section>
		{:else if selected?.kind === 'text'}<section class="public-text-tab">
				<h2>{selected.label}</h2>
				<p>{selected.body || 'No additional information has been published.'}</p>
			</section>{/if}
		{#if !data.tabs.some((tab) => tab.kind === 'overview' || tab.kind === 'metrics')}
			<PublicGraphs summary={data.metrics_summary} settings={metricsSettings} />
		{/if}
		<div class="public-footnote">
			<RefreshCw size={12} /> Last verified {time(updated)} · Refreshes every 30 seconds · Times shown in {timezone}
		</div>
		<footer class="public-footer">
			<span><ShieldCheck size={14} /> Independently monitored</span><span>Powered by {data.brand}</span>
		</footer>
	</main>
</div>
