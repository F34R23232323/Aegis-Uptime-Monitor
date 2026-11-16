<script lang="ts" module>
	function latencyPercentileFrom(values: number[], p: number) {
		if (!values.length) return '—';
		values.sort((a, b) => a - b);
		return Math.round(
			values[Math.min(values.length - 1, Math.ceil((values.length * p) / 100) - 1)]
		).toString();
	}
</script>

<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import {
		ShieldCheck,
		LayoutDashboard,
		Radio,
		Server as ServerIcon,
		TriangleAlert,
		Bell,
		Globe,
		ChartNoAxesCombined,
		Settings,
		Wrench,
		Plus,
		Search,
		ArrowUpRight,
		ArrowRight,
		ChevronRight,
		ChevronDown,
		Activity,
		Clock,
		Check,
		Pause,
		Play,
		Trash2,
		ExternalLink,
		Menu,
		X,
		LogOut,
		Copy,
		Terminal,
		Container,
		Layers,
		RefreshCw,
		SlidersHorizontal,
		CheckCheck,
		Command,
		Download,
		Ellipsis,
		Signal,
		LockKeyhole
	} from '@lucide/svelte';
	import {
		api,
		time,
		relative,
		duration,
		percent,
		monitorStatus,
		type Monitor,
		type Check as Sample,
		type Incident,
		type Server,
		type Page,
		type Uptime,
		type User,
		type Service
	} from './api';
	import Auth from './Auth.svelte';
	import Status from './Status.svelte';
	import Chart from './Chart.svelte';
	import Bars from './Bars.svelte';
	import Modal from './Modal.svelte';
	import MonitorForm from './MonitorForm.svelte';
	import NotificationsPanel from './NotificationsPanel.svelte';
	import StatusStudio from './StatusStudio.svelte';
	import PresetCatalog from './PresetCatalog.svelte';
	import PresetFolders from './PresetFolders.svelte';
	import IncidentsPanel from './IncidentsPanel.svelte';
	import MaintenancePanel from './MaintenancePanel.svelte';
	import SecurityPanel from './SecurityPanel.svelte';
	import ServersOverview from './ServersOverview.svelte';
	let loading = $state(true);
	let user = $state<User | null>(null);
	let setup = $state(false);
	let requiresSetupToken = $state(false);
	let brand = $state('Aegis Monitor');
	let fatal = $state('');
	let monitors = $state<Monitor[]>([]);
	let monitorOffset = $state(0);
	let monitorCounts = $state<{ total: number; states: Record<string, number>; open_incidents: number }>({
		total: 0,
		states: {},
		open_incidents: 0
	});
	let incidents = $state<Incident[]>([]);
	let servers = $state<Server[]>([]);
	let pages = $state<Page[]>([]);
	let histories = $state<Record<string, Sample[]>>({});
	let uptimes = $state<Record<string, Uptime>>({});
	let detailChecks = $state<Sample[]>([]);
	let detailUptime = $state<Uptime[]>([]);
	let connected = $state(false);
	let search = $state('');
	let filter = $state('all');
	let typeFilter = $state('all');
	let grid = $state(false);
	let mobile = $state(false);
	let toast = $state('');
	let addOpen = $state(false);
	let editing = $state<Partial<Monitor> | undefined>();
	let serverOpen = $state(false);
	let serverName = $state('');
	let enrollment = $state<{ token: string; id?: string } | null>(null);
	let busy = $state(false);
	let formError = $state('');
	let confirmation = $state<{ message: string; action: () => Promise<void> } | null>(null);
	let currentPassword = $state('');
	let newPassword = $state('');
	let members = $state<User[]>([]);
	let memberEmail = $state('');
	let memberPassword = $state('');
	let memberRole = $state('viewer');
	let system = $state<Record<string, unknown>>({});
	let analyticsDays = $state(1);
	let eventSource: EventSource | undefined;
	let refreshTimer: ReturnType<typeof setTimeout> | undefined;
	let toastTimer: ReturnType<typeof setTimeout> | undefined;
	const nav = [
		{ path: '/', label: 'Overview', icon: LayoutDashboard },
		{ path: '/monitors', label: 'Monitors', icon: Radio },
		{ path: '/presets', label: 'Premade monitors', icon: Layers },
		{ path: '/servers', label: 'Servers', icon: ServerIcon },
		{ path: '/incidents', label: 'Incidents', icon: TriangleAlert },
		{ path: '/analytics', label: 'Analytics', icon: ChartNoAxesCombined },
		{ path: '/notifications', label: 'Notifications', icon: Bell },
		{ path: '/status-pages', label: 'Status pages', icon: Globe },
		{ path: '/maintenance', label: 'Maintenance', icon: Wrench }
	];
	let route = $derived(page.url.pathname);
	let detail = $derived(
		route.startsWith('/monitors/') ? monitors.find((m) => m.id === route.split('/')[2]) : undefined
	);
	let active = $derived(nav.find((n) => n.path === route));
	let down = $derived(monitorCounts.states.DOWN || 0);
	let up = $derived(monitorCounts.states.UP || 0);
	let openIncidents = $derived(incidents.filter((i) => !i.ended_at));
	let filtered = $derived(
		monitors.filter(
			(m) =>
				(filter === 'all' || m.state === filter) &&
				(typeFilter === 'all' || m.kind === typeFilter) &&
				`${m.name} ${m.target} ${m.group_name} ${m.tags.join(' ')}`
					.toLowerCase()
					.includes(search.toLowerCase())
		)
	);
	let presetFiltered = $derived(filtered.filter((m) => m.tags.includes('preset')));
	let regularFiltered = $derived(filtered.filter((m) => !m.tags.includes('preset')));
	let averageLatency = $derived(
		monitors
			.filter((m) => m.last_latency != null && ['http', 'tcp'].includes(m.kind))
			.reduce((a, m, _, arr) => a + (m.last_latency || 0) / arr.length, 0)
	);
	let overallUptime = $derived.by(() => {
		let up = 0,
			down = 0;
		Object.values(uptimes).forEach((v) => {
			up += v.up_seconds;
			down += v.down_seconds;
		});
		return up + down ? (100 * up) / (up + down) : null;
	});
	function notify(s: string) {
		toast = s;
		clearTimeout(toastTimer);
		toastTimer = setTimeout(() => (toast = ''), 6000);
	}
	async function initialize() {
		loading = true;
		fatal = '';
		try {
			const state = await api<{
				needs_setup: boolean;
				requires_setup_token: boolean;
				name: string;
				accent: string;
			}>('/setup');
			setup = state.needs_setup;
			requiresSetupToken = state.requires_setup_token;
			brand = state.name;
			document.documentElement.style.setProperty('--accent', state.accent);
			if (!setup) {
				try {
					user = await api<User>('/me');
				} catch {
					user = null;
				}
			}
			if (user) {
				await refresh();
				connect();
			}
		} catch (e) {
			fatal = (e as Error).message;
		} finally {
			loading = false;
		}
	}
	function connect() {
		eventSource?.close();
		eventSource = new EventSource('/api/v1/events');
		eventSource.onopen = () => (connected = true);
		eventSource.onerror = () => (connected = false);
		eventSource.onmessage = () => {
			clearTimeout(refreshTimer);
			refreshTimer = setTimeout(() => void refresh().catch((e) => notify(e.message)), 450);
		};
	}
	async function refresh() {
		if (route === '/') monitorOffset = 0;
		const values = await Promise.all([
			api<Monitor[]>(`/monitors?limit=500&offset=${monitorOffset}`),
			api<Incident[]>('/incidents'),
			api<Server[]>('/servers'),
			api<Page[]>('/status-pages')
		]);
		[monitors, incidents, servers, pages] = values;
		monitorCounts = await api('/monitors/counts');

		const data = await api<{ id: string; checks: Sample[]; uptime: Uptime }[]>(
			'/monitors/summaries?days=' + analyticsDays
		);
		histories = Object.fromEntries(data.map((d) => [d.id, d.checks]));
		uptimes = Object.fromEntries(data.map((d) => [d.id, d.uptime]));
		if (detail) await loadDetail(detail.id);
	}
	async function loadDetail(id: string) {
		[detailChecks, detailUptime] = await Promise.all([
			api<Sample[]>(`/monitors/${id}/checks?limit=240`),
			Promise.all([1, 7, 30, 90].map((d) => api<Uptime>(`/monitors/${id}/uptime?days=${d}`)))
		]);
	}
	$effect(() => {
		if (detail?.id && user) void loadDetail(detail.id).catch((e) => notify(e.message));
	});
	$effect(() => {
		if (route === '/settings' && user) {
			void api<Record<string, unknown>>('/system').then((v) => (system = v));
			if (user.role === 'owner') void api<User[]>('/users').then((v) => (members = v));
		}
	});
	onMount(() => {
		void initialize();
		const fallback = setInterval(() => {
			if (user) void refresh().catch(() => (connected = false));
		}, 30000);
		const key = (e: KeyboardEvent) => {
			if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
				e.preventDefault();
				void goto('/monitors').then(() => document.getElementById('monitor-search')?.focus());
			}
			if (e.key === 'Escape') mobile = false;
		};
		window.addEventListener('keydown', key);
		return () => {
			eventSource?.close();
			clearInterval(fallback);
			clearTimeout(refreshTimer);
			clearTimeout(toastTimer);
			window.removeEventListener('keydown', key);
		};
	});
	async function action(fn: () => Promise<unknown>, message = 'Saved') {
		busy = true;
		formError = '';
		try {
			await fn();
			await refresh();
			notify(message);
		} catch (e) {
			formError = (e as Error).message;
			notify(formError);
		} finally {
			busy = false;
		}
	}
	function newMonitor(m?: Monitor) {
		editing = m;
		addOpen = true;
	}
	async function saved() {
		addOpen = false;
		editing = undefined;
		await refresh();
	}
	function confirm(message: string, fn: () => Promise<void>) {
		confirmation = { message, action: fn };
	}
	async function removeMonitor(m: Monitor) {
		await api('/monitors/' + m.id, 'DELETE');
		await refresh();
		if (detail) await goto('/monitors');
		notify('Monitor deleted');
	}
	async function addServer(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		formError = '';
		try {
			enrollment = await api('/servers', 'POST', { name: serverName });
			await refresh();
		} catch (e) {
			formError = (e as Error).message;
		} finally {
			busy = false;
		}
	}
	async function discover(s: Server, item?: Service, kind = 'host') {
		editing = {
			name: item?.name.replace(/^\//, '') || s.name + ' health',
			kind,
			target: item?.id || s.id,
			server_id: s.id,
			interval_seconds: 30,
			timeout_seconds: 10,
			failure_threshold: 2,
			recovery_threshold: 2,
			group_name: s.name,
			description: '',
			tags: [],
			public: false,
			config: {}
		}; // no id: creation form
		addOpen = true;
	}
	async function copy(text: string) {
		try {
			await navigator.clipboard.writeText(text);
			notify('Copied to clipboard');
		} catch {
			notify('Clipboard unavailable. Select and copy the text.');
		}
	}
	async function logout() {
		await api('/auth/logout', 'POST');
		user = null;
		eventSource?.close();
		await goto('/');
	}
	function latencyPercentile(p: number) {
		return latencyPercentileFrom(
			Object.values(histories)
				.flat()
				.filter((c) => c.ok && c.latency_ms > 0)
				.map((c) => c.latency_ms),
			p
		);
	}
	function exportData() {
		const blob = new Blob([JSON.stringify({ monitors, incidents, servers, status_pages: pages }, null, 2)], {
			type: 'application/json'
		});
		const url = URL.createObjectURL(blob);
		const a = document.createElement('a');
		a.href = url;
		a.download = 'aegis-export.json';
		a.click();
		URL.revokeObjectURL(url);
	}
</script>

<svelte:head
	><title
		>{detail?.name || active?.label || (route === '/settings' ? 'Settings' : 'Overview')} · {brand}</title
	><meta name="description" content="Your self-hosted infrastructure command center." /></svelte:head
>
{#if loading}<div class="boot">
		<span class="brand-mark"><ShieldCheck size={32} /></span>
		<div class="loading-line"></div>
		<p>Connecting to your command center…</p>
	</div>
{:else if fatal}<main class="boot">
		<TriangleAlert size={36} />
		<h1>Connection interrupted</h1>
		<p class="muted">{fatal}</p>
		<button class="button primary" onclick={initialize}>Try again</button>
	</main>
{:else if !user}<Auth {setup} {requiresSetupToken} name={brand} ondone={initialize} />
{:else}
	<div class="app-shell">
		{#if mobile}<button class="mobile-scrim" aria-label="Close navigation" onclick={() => (mobile = false)}
			></button>{/if}
		<aside class:mobile-open={mobile} class="sidebar">
			<a href="/" class="brand" onclick={() => (mobile = false)}
				><span class="brand-mark"><ShieldCheck size={22} /></span><span
					>{brand}<small>INFRASTRUCTURE MONITOR</small></span
				></a
			>
			<div class="workspace">
				<span class="workspace-avatar">P</span>
				<div>Personal workspace<small>Self-hosted instance</small></div>
				<ChevronDown size={15} />
			</div>
			<div class="nav-label">WORKSPACE</div>
			<nav>
				{#each nav as n}<a
						href={n.path}
						class:active={route === n.path || (n.path === '/monitors' && !!detail)}
						onclick={() => (mobile = false)}
						><n.icon size={18} />{n.label}{#if n.path === '/monitors'}<span class="nav-count"
								>{monitors.length}</span
							>{:else if n.path === '/incidents' && openIncidents.length}<span class="nav-count danger-count"
								>{monitorCounts.open_incidents}</span
							>{/if}</a
					>{/each}
			</nav>
			<div class="sidebar-bottom">
				<div class="observer-note">
					<span class="tiny-label">INDEPENDENT OBSERVATION</span>
					<p>Run Aegis outside the servers it monitors to detect a total outage.</p>
					<a href="/servers">Connect a server <ArrowUpRight size={13} /></a>
				</div>
				<a class:active={route === '/settings'} class="settings-link" href="/settings"
					><Settings size={18} />Settings</a
				>
				<div class="profile">
					<span class="profile-avatar">{user.email.slice(0, 1).toUpperCase()}</span>
					<div>{user.email.split('@')[0]}<small>{user.role}</small></div>
					<button class="icon-button" aria-label="Sign out" onclick={() => void logout()}
						><LogOut size={16} /></button
					>
				</div>
			</div>
		</aside>
		<div class="main-shell">
			<header class="topbar">
				<div class="breadcrumbs">
					<button class="icon-button mobile-menu" aria-label="Open navigation" onclick={() => (mobile = true)}
						><Menu size={20} /></button
					><span>Workspace</span><ChevronRight size={13} /><strong
						>{detail ? 'Monitors' : active?.label || 'Settings'}</strong
					>{#if detail}<ChevronRight size={13} /><strong>{detail.name}</strong>{/if}
				</div>
				<div class="topbar-right">
					<span class:disconnected={!connected} class="live-indicator"
						><span class="live-dot"></span>{connected ? 'Live updates' : 'Reconnecting'}</span
					><button
						class="search-shortcut"
						onclick={() =>
							void goto('/monitors').then(() => document.getElementById('monitor-search')?.focus())}
						><Search size={15} /><span>Search anything</span><kbd>⌘ K</kbd></button
					><button
						class="icon-button notifications-button"
						aria-label="View incidents"
						onclick={() => void goto('/incidents')}
						><Bell size={18} />{#if openIncidents.length}<i></i>{/if}</button
					>
				</div>
			</header>
			<main class="main-content">
				{#if route === '/' || route === '/monitors'}
					<div class="page-heading">
						<div>
							<div class="eyebrow">{route === '/' ? 'THE BIG PICTURE' : 'EVERY ENDPOINT. ONE PLACE.'}</div>
							<h1>{route === '/' ? 'Infrastructure, in harmony.' : 'Your monitors.'}</h1>
							<p class="muted">
								{route === '/'
									? 'A clear view of what’s running, what’s responding, and what needs you.'
									: 'Real checks, meaningful signals, and nothing lost in the noise.'}
							</p>
						</div>
						{#if user.role !== 'viewer'}<button class="button primary" onclick={() => newMonitor()}
								><Plus size={17} /> Add monitor</button
							>{/if}
					</div>
					{#if route === '/'}
						<section class="health-banner" class:has-outage={down > 0}>
							<div class="health-emblem">
								{#if down}<TriangleAlert
										size={28}
									/>{:else if monitors.length && up === monitorCounts.total}<CheckCheck
										size={28}
									/>{:else}<Activity size={28} />{/if}
							</div>
							<div>
								<div class="health-title">
									{down
										? `${down} ${down === 1 ? 'monitor needs' : 'monitors need'} your attention`
										: monitors.length && up === monitorCounts.total
											? 'All systems operational'
											: monitors.length
												? 'Your infrastructure is taking shape'
												: 'Your next quiet night starts here.'}<span class="pill"
										>{monitors.length ? `${up} / ${monitorCounts.total} healthy` : 'LET’S GET STARTED'}</span
									>
								</div>
								<p>
									{down
										? 'We’re tracking the incident. Check the timeline for the latest observations.'
										: monitors.length
											? 'Independent checks are running in the background. We’ll keep you informed.'
											: 'Add your first monitor and let Aegis keep watch.'}
								</p>
							</div>
							<div class="banner-signal">
								<span></span><span></span><span></span><span></span><span></span><span></span><span
								></span><span></span><span></span>
							</div>
						</section>
						<div class="stats-grid">
							<div class="stat-card">
								<div class="stat-top">Total monitors<Radio size={16} /></div>
								<strong>{monitorCounts.total}<span class="stat-unit">monitors</span></strong>
								<div class="stat-bottom">
									<span class="green"><span class="tiny-dot"></span> {up} operational</span><span
										>{monitorCounts.states.PAUSED || 0} paused</span
									>
								</div>
							</div>
							<div class="stat-card">
								<div class="stat-top">Observed uptime<ChartNoAxesCombined size={16} /></div>
								<strong
									>{overallUptime == null ? '—' : overallUptime.toFixed(2)}<span class="stat-unit"
										>{overallUptime == null ? 'No coverage' : '%'}</span
									></strong
								>
								<div class="stat-bottom">
									<span class="green">{'Latest 100 monitors · duration-weighted'}</span><span>24 hours</span>
								</div>
							</div>
							<div class="stat-card">
								<div class="stat-top">Average response<Activity size={16} /></div>
								<strong
									>{averageLatency ? Math.round(averageLatency) : '—'}<span class="stat-unit">ms</span
									></strong
								>
								<div class="stat-bottom">
									<span>Latest HTTP & TCP checks</span><span class="mini-wave">⌁⌁⌁</span>
								</div>
							</div>
							<div class="stat-card">
								<div class="stat-top">Active incidents<TriangleAlert size={16} /></div>
								<strong class:rose={monitorCounts.open_incidents > 0}
									>{monitorCounts.open_incidents}<span class="stat-unit">open</span></strong
								>
								<div class="stat-bottom">
									<span
										>{openIncidents.length
											? 'Investigate current interruptions'
											: 'Looking calm across the board'}</span
									><a href="/incidents" aria-label="View incidents"><ArrowUpRight size={15} /></a>
								</div>
							</div>
						</div>
					{/if}
					<div class="section-heading">
						<div class="section-title">
							<h2>{route === '/' ? 'Monitor overview' : 'All monitors'}</h2>
							<span class="count-badge">{monitors.length}</span>
						</div>
						<div class="section-tools">
							{#if route === '/'}<a class="subtle-link" href="/monitors"
									>View all monitors <ArrowRight size={14} /></a
								>{:else}<button class="button small" onclick={() => (grid = !grid)}
									><Layers size={14} />{grid ? 'Table view' : 'Card view'}</button
								>{/if}
						</div>
					</div>
					{#if monitorCounts.total > 500}<div class="notice">
							Showing monitors {monitorOffset + 1}–{monitorOffset + monitors.length} of {monitorCounts.total}.
							Search and filters apply to this page; global health counts include every monitor.
						</div>{/if}
					<section class="panel monitor-panel">
						<div class="monitor-toolbar">
							<div class="filter-tabs">
								{#each [['all', 'All monitors'], ['UP', 'Operational'], ['DOWN', 'Down'], ['PAUSED', 'Paused']] as [value, label]}<button
										class:chosen={filter === value}
										onclick={() => (filter = value)}
										>{label}{#if value === 'all'}<span>{monitors.length}</span>{/if}</button
									>{/each}
							</div>
							<div class="monitor-controls">
								<div class="search-field">
									<Search size={15} /><input
										id="monitor-search"
										bind:value={search}
										placeholder="Search monitors…"
										aria-label="Search monitors"
									/>
								</div>
								<select class="type-filter" aria-label="Filter by monitor type" bind:value={typeFilter}
									><option value="all">All types</option
									>{#each ['http', 'json', 'tcp', 'dns', 'tls', 'redis', 'smtp', 'postgres', 'push', 'docker', 'pm2', 'host', 'cpu', 'memory', 'disk'] as t}<option
											value={t}>{t.toUpperCase()}</option
										>{/each}</select
								>
							</div>
						</div>
						{#if !monitors.length}<div class="empty-state">
								<div class="empty-icon"><Radio size={29} /></div>
								<h3>Give your services a watchful eye.</h3>
								<p>
									Add a website, TCP port, or connect a server.<br />Your first real signal is one monitor
									away.
								</p>
								{#if user.role !== 'viewer'}<button class="button primary" onclick={() => newMonitor()}
										><Plus size={16} /> Create your first monitor</button
									>{/if}<span class="muted small-text"
									>Checks run continuously, even when you close this tab.</span
								>
							</div>
						{:else if !filtered.length}<div class="empty-state">
								<Search size={26} />
								<h3>No matching monitors</h3>
								<button
									class="text-button"
									onclick={() => {
										search = '';
										filter = 'all';
										typeFilter = 'all';
									}}>Clear filters</button
								>
							</div>
						{:else if grid}<PresetFolders monitors={presetFiltered} {search} />
							{#if regularFiltered.length}<div class="monitor-grid">
									{#each regularFiltered as m}<a href={'/monitors/' + m.id} class="monitor-card"
											><div class="row-between">
												<span class="kind-icon"><Radio size={18} /></span><Status
													{...monitorStatus(m)}
													small
												/>
											</div>
											<h3>{m.name}</h3>
											<p class="muted truncate">{m.target || m.kind}</p>
											<Chart checks={histories[m.id] || []} compact />
											<div class="row-between">
												<strong>{percent(uptimes[m.id]?.uptime)}<small> availability</small></strong><span
													>{m.last_latency?.toFixed(0) || '—'} ms</span
												>
											</div>
											<Bars history={histories[m.id] || []} /></a
										>{/each}
								</div>{/if}
						{:else}<PresetFolders monitors={presetFiltered} {search} />
							{#if regularFiltered.length}<div class="table-scroll">
									<table class="monitors-table">
										<thead
											><tr
												><th>Monitor</th><th>Status</th><th>Response time</th><th>Uptime · 24h</th><th
													>Recent checks</th
												><th></th></tr
											></thead
										><tbody
											>{#each regularFiltered as m}<tr
													><td
														><a class="monitor-name-cell" href={'/monitors/' + m.id}
															><span class="kind-icon"
																>{#if m.kind === 'docker'}<Container
																		size={17}
																	/>{:else if m.kind === 'host'}<ServerIcon
																		size={17}
																	/>{:else if m.kind === 'http'}<Globe size={17} />{:else}<Radio
																		size={17}
																	/>{/if}</span
															><span
																><strong>{m.name}</strong><small
																	>{m.group_name || m.target || m.kind.toUpperCase()}</small
																></span
															></a
														></td
													><td><Status {...monitorStatus(m)} small /></td><td
														><div class="latency-cell">
															<span
																>{m.last_latency == null ? '—' : Math.round(m.last_latency)}
																<small>ms</small></span
															><Chart checks={histories[m.id] || []} compact />
														</div></td
													><td><span class="uptime-value">{percent(uptimes[m.id]?.uptime)}</span></td><td
														><Bars history={histories[m.id] || []} /></td
													><td
														><a class="icon-button" aria-label={'Open ' + m.name} href={'/monitors/' + m.id}
															><ChevronRight size={17} /></a
														></td
													></tr
												>{/each}</tbody
										>
									</table>
								</div>{/if}{/if}
						<div class="panel-footer">
							<span><span class="live-dot"></span> Checks run independently of your browser</span><span
								>UTC storage · local display</span
							>
						</div>
						{#if route === '/monitors' && monitorCounts.total > 500}<div class="panel-footer">
								<button
									class="button"
									disabled={monitorOffset === 0}
									onclick={() => {
										monitorOffset = Math.max(0, monitorOffset - 500);
										void refresh();
									}}>Newer</button
								>
								<span
									>Page {Math.floor(monitorOffset / 500) + 1} of {Math.ceil(monitorCounts.total / 500)}</span
								>
								<button
									class="button"
									disabled={monitorOffset + monitors.length >= monitorCounts.total}
									onclick={() => {
										monitorOffset += 500;
										void refresh();
									}}>Older</button
								>
							</div>{/if}
					</section>
					{#if route === '/'}<div class="bottom-grid">
							<section class="panel">
								<div class="panel-heading">
									<div class="section-title">
										<Clock size={17} />
										<h2>Recent incidents</h2>
									</div>
									<a href="/incidents" class="subtle-link">View history <ArrowUpRight size={14} /></a>
								</div>
								{#if !incidents.length}<div class="calm-state">
										<span class="calm-icon"><CheckCheck size={24} /></span>
										<h3>A little peace of mind.</h3>
										<p>No incidents recorded. We’ll keep watch.</p>
									</div>{:else}<div class="activity-list">
										{#each incidents.slice(0, 4) as i}<a
												href={i.monitor_id ? '/monitors/' + i.monitor_id : '/incidents'}
												class="activity-item"
												><span class:resolved={!!i.ended_at} class="event-icon"
													>{#if i.ended_at}<Check size={14} />{:else}<TriangleAlert size={14} />{/if}</span
												>
												<div>
													<strong>{i.title || i.monitor_name || 'Incident'}</strong><small
														>{i.ended_at ? 'Recovered' : 'Incident ongoing'} · {duration(
															i.started_at,
															i.ended_at
														)}</small
													>
												</div>
												<span class="muted">{relative(i.started_at)}</span></a
											>{/each}
									</div>{/if}
							</section>
							<section class="panel">
								<div class="panel-heading">
									<div class="section-title">
										<ServerIcon size={17} />
										<h2>Your servers</h2>
									</div>
									<a class="subtle-link" href="/servers">Manage <ArrowUpRight size={14} /></a>
								</div>
								{#if !servers.length}<div class="calm-state">
										<span class="calm-icon violet"><ServerIcon size={24} /></span>
										<h3>A home for every machine.</h3>
										<p>Connect an agent for Docker, PM2 and host health.</p>
										<a href="/servers" class="subtle-link accent"
											>Connect your first server <ArrowRight size={14} /></a
										>
									</div>{:else}<div class="activity-list">
										{#each servers.slice(0, 4) as s}<a href="/servers" class="activity-item"
												><span class="kind-icon"><ServerIcon size={17} /></span>
												<div>
													<strong>{s.name}</strong><small
														>{s.inventory.host
															? `CPU ${s.inventory.host.cpu_percent.toFixed(0)}% · RAM ${s.inventory.host.memory_percent.toFixed(0)}%`
															: 'Waiting for heartbeat'}</small
													>
												</div>
												<Status
													state={s.last_seen && Date.now() - new Date(s.last_seen).getTime() < 60000
														? 'UP'
														: 'UNKNOWN'}
													small
												/></a
											>{/each}
									</div>{/if}
							</section>
						</div>{/if}
				{:else if detail}
					<div class="page-heading">
						<div>
							<a class="subtle-link" href="/monitors">← All monitors</a>
							<h1 class="detail-title">{detail.name}<Status {...monitorStatus(detail)} /></h1>
							<p class="muted">
								{detail.target || detail.kind} <span class="dot-separator">·</span> Every {detail.interval_seconds}s
								<span class="dot-separator">·</span>
								{relative(detail.last_checked_at)}
							</p>
						</div>
						{#if user.role !== 'viewer'}<div class="button-group">
								<button
									class="button"
									onclick={() =>
										void action(
											() => api('/monitors/' + detail!.id + '/toggle', 'POST'),
											detail!.enabled ? 'Monitor paused' : 'Monitor resumed'
										)}
									>{#if detail.enabled}<Pause size={15} />Pause{:else}<Play size={15} />Resume{/if}</button
								><button class="button primary" onclick={() => newMonitor(detail)}
									><SlidersHorizontal size={15} />Edit monitor</button
								><button
									class="icon-button danger"
									aria-label="Delete monitor"
									onclick={() =>
										confirm('Permanently delete this monitor, its checks and incidents?', () =>
											removeMonitor(detail!)
										)}><Trash2 size={17} /></button
								>
							</div>{/if}
					</div>
					{#if detail.last_error}<div class="error detail-error">
							<TriangleAlert size={18} /><span
								>{detail.last_error}{#if detail.kind === 'pm2' && detail.state === 'UP' && detail.last_error.startsWith('PM2 process is offline')}
									· Confirming failure {detail.failures || 1} of {detail.failure_threshold} before an incident is
									opened.{/if}</span
							>
						</div>{/if}
					<div class="stats-grid">
						{#each detailUptime as u}<div class="stat-card">
								<div class="stat-top">
									Uptime · {u.days === 1 ? '24 hours' : u.days + ' days'}<Clock size={15} />
								</div>
								<strong>{percent(u.uptime)}</strong>
								<div class="stat-bottom">
									{Math.round(u.covered_seconds / 60)} minutes of observed coverage
								</div>
							</div>{/each}
					</div>
					<section class="panel chart-panel">
						<div class="panel-heading">
							<div>
								<h2>Response latency</h2>
								<p class="muted small-text">Most recent {detailChecks.length} observations · milliseconds</p>
							</div>
							<span class="metric-highlight">{detail.last_latency?.toFixed(1) || '—'} <small>ms</small></span>
						</div>
						<Chart checks={detailChecks} />
					</section>
					<section class="panel history-panel">
						<div class="panel-heading">
							<h2>Availability history</h2>
							<span class="muted small-text">Latest 60 checks · hover for timestamps</span>
						</div>
						<Bars history={detailChecks} />
						<div class="row-between muted small-text"><span>Earlier</span><span>Now</span></div>
					</section>
					<div class="bottom-grid">
						<section class="panel">
							<div class="panel-heading">
								<h2>Check log</h2>
								<span class="count-badge">{detailChecks.length}</span>
							</div>
							<div class="check-log">
								{#each [...detailChecks].reverse().slice(0, 30) as c}<div class="check-row">
										<Status state={c.ok ? 'UP' : 'DOWN'} small />
										<div><strong>{c.error || 'Check passed'}</strong><small>{time(c.created_at)}</small></div>
										<span>{c.latency_ms.toFixed(1)} ms</span>
									</div>{:else}<div class="calm-state">Waiting for the scheduler’s first check.</div>{/each}
							</div>
						</section>
						<section class="panel">
							<div class="panel-heading"><h2>Incident history</h2></div>
							{#each incidents.filter((i) => i.monitor_id === detail?.id) as i}<div class="incident-mini">
									<div class="row-between">
										<strong>{i.ended_at ? 'Resolved' : 'Ongoing'}</strong><span
											>{duration(i.started_at, i.ended_at)}</span
										>
									</div>
									<p class="muted">{i.cause}</p>
									<small>{time(i.started_at)}</small>
								</div>{:else}<div class="calm-state">
									<CheckCheck size={25} />
									<h3>No incidents yet</h3>
									<p>Confirmed downtime will appear here.</p>
								</div>{/each}
							<div class="detail-meta">
								<h3>Configuration</h3>
								<dl>
									<dt>Type</dt>
									<dd>{detail.kind.toUpperCase()}</dd>
									<dt>Failure / recovery threshold</dt>
									<dd>{detail.failure_threshold} / {detail.recovery_threshold}</dd>
									<dt>Group</dt>
									<dd>{detail.group_name || 'Ungrouped'}</dd>
									<dt>Visibility</dt>
									<dd>{detail.public ? 'Public eligible' : 'Private'}</dd>
									<dt>Alerts</dt>
									<dd>Configured notification routes</dd>
								</dl>
								{#if detail.description}<p class="muted">{detail.description}</p>{/if}
								<div class="tag-list">
									{#each detail.tags as tag}<span class="tag">{tag}</span>{/each}
								</div>
							</div>
						</section>
					</div>
				{:else if route === '/servers'}
					<div class="page-heading">
						<div>
							<div class="eyebrow">THE MACHINES BEHIND THE MAGIC</div>
							<h1>Your infrastructure.</h1>
							<p class="muted">Host health, containers, and processes. A direct line to every server.</p>
						</div>
						{#if user.role !== 'viewer'}<button
								class="button primary"
								onclick={() => {
									serverOpen = true;
									enrollment = null;
									serverName = '';
								}}><Plus size={16} /> Connect server</button
							>{/if}
					</div>
					<div class="notice observer-info">
						<ShieldCheck size={19} /><span
							>For complete outage detection, host Aegis on an independent machine. An agent cannot report
							while its own server is offline. Create a host monitor for heartbeat alerts.</span
						>
					</div>
					{#if !servers.length}<section class="panel empty-state">
							<div class="empty-icon"><ServerIcon size={30} /></div>
							<h3>Every server deserves a pulse.</h3>
							<p>
								Enroll an outbound-only Rust agent. No inbound ports required.<br />Discover Docker containers
								and PM2 apps, and watch real host metrics.
							</p>
							{#if user.role !== 'viewer'}<button
									class="button primary"
									onclick={() => {
										serverOpen = true;
										enrollment = null;
									}}><Plus size={16} /> Connect your first server</button
								>{/if}
						</section>{/if}
					<ServersOverview
						{servers}
						{monitors}
						readonly={user.role === 'viewer'}
						ondiscover={(server, item, kind) => void discover(server, item, kind)}
						onrotate={(server) =>
							confirm(
								'Rotate this agent credential? The existing token will stop working immediately.',
								async () => {
									enrollment = await api('/servers/' + server.id + '/rotate', 'POST');
									serverOpen = true;
								}
							)}
						onrevoke={(server) =>
							confirm('Revoke and delete this server? Remove associated monitors first.', async () => {
								await api('/servers/' + server.id, 'DELETE');
								await refresh();
							})}
					/>
				{:else if route === '/incidents'}
					<IncidentsPanel readonly={user.role === 'viewer'} onmessage={notify} />
				{:else if route === '/notifications'}
					<NotificationsPanel readonly={user.role === 'viewer'} onmessage={notify} />
				{:else if route === '/status-pages'}
					<StatusStudio readonly={user.role === 'viewer'} onmessage={notify} />
				{:else if route === '/presets'}
					<PresetCatalog readonly={user.role === 'viewer'} onmessage={notify} oncreated={refresh} />
				{:else if route === '/maintenance'}
					<MaintenancePanel readonly={user.role === 'viewer'} onmessage={notify} />
				{:else if route === '/analytics'}
					<div class="page-heading">
						<div>
							<div class="eyebrow">A LITTLE PERSPECTIVE</div>
							<h1>Reliability, measured.</h1>
							<p class="muted">
								Real observations over time. Unknown and paused periods never count as uptime.
							</p>
						</div>
						<select aria-label="Analytics period" bind:value={analyticsDays} onchange={() => void refresh()}
							><option value={1}>24 hours</option><option value={7}>7 days</option><option value={30}
								>30 days</option
							><option value={90}>90 days</option></select
						>
					</div>
					<div class="stats-grid">
						<div class="stat-card">
							<div class="stat-top">Combined observed uptime</div>
							<strong>{percent(overallUptime)}</strong><small class="muted"
								>Weighted by covered duration</small
							>
						</div>
						<div class="stat-card">
							<div class="stat-top">Recorded incidents</div>
							<strong
								>{incidents.filter(
									(i) => Date.now() - new Date(i.started_at).getTime() < analyticsDays * 86400000
								).length}</strong
							><small class="muted">Within selected period, latest 200 incidents</small>
						</div>
						<div class="stat-card">
							<div class="stat-top">p50 latency</div>
							<strong>{latencyPercentile(50)} <span class="stat-unit">ms</span></strong><small class="muted"
								>Latest 60 observations per monitor</small
							>
						</div>
						<div class="stat-card">
							<div class="stat-top">p95 latency</div>
							<strong>{latencyPercentile(95)} <span class="stat-unit">ms</span></strong><small class="muted"
								>Latest 60 observations per monitor</small
							>
						</div>
					</div>
					<section class="panel">
						<div class="panel-heading">
							<h2>Monitor reliability</h2>
							<span class="muted small-text"
								>{analyticsDays} day{analyticsDays === 1 ? '' : 's'} · coverage-aware</span
							>
						</div>
						<div class="table-scroll">
							<table>
								<thead
									><tr
										><th>Monitor</th><th>Availability</th><th>Covered time</th><th>Down time</th><th
											>Recent latency</th
										></tr
									></thead
								><tbody
									>{#each [...monitors]
										.sort((a, b) => (a.last_latency || 0) - (b.last_latency || 0))
										.reverse() as m}<tr
											><td
												><a href={'/monitors/' + m.id}>{m.name}</a><small class="muted"
													>{m.kind} · {m.group_name || 'Ungrouped'}</small
												></td
											><td class="green">{percent(uptimes[m.id]?.uptime)}</td><td
												>{((uptimes[m.id]?.covered_seconds || 0) / 3600).toFixed(2)} h</td
											><td>{((uptimes[m.id]?.down_seconds || 0) / 60).toFixed(1)} m</td><td
												><Chart checks={histories[m.id] || []} compact /></td
											></tr
										>{:else}<tr
											><td class="empty-cell" colspan="5">Add a monitor to begin measuring reliability.</td
											></tr
										>{/each}</tbody
								>
							</table>
						</div>
					</section>
				{:else if route === '/settings'}
					<div class="page-heading">
						<div>
							<div class="eyebrow">MAKE YOURSELF AT HOME</div>
							<h1>Instance settings.</h1>
							<p class="muted">Your account, your security, your preferences.</p>
						</div>
						<button class="button" onclick={exportData}><Download size={16} /> Export data</button>
					</div>
					<div class="settings-grid">
						<SecurityPanel owner={user.role === 'owner'} onmessage={notify} />
						<section class="panel settings-panel">
							<h2>Profile & security</h2>
							<div class="profile-summary">
								<span class="profile-avatar">{user.email[0].toUpperCase()}</span>
								<div>
									<strong>{user.email}</strong><small class="muted"
										>{user.role} · Password authentication</small
									>
								</div>
							</div>
							<form
								class="form-stack"
								onsubmit={(e) => {
									e.preventDefault();
									void action(async () => {
										await api('/auth/password', 'POST', { current: currentPassword, password: newPassword });
										user = null;
										eventSource?.close();
									}, 'Password changed. All sessions have been revoked.');
								}}
							>
								<label
									>Current password<input
										type="password"
										autocomplete="current-password"
										bind:value={currentPassword}
										required
									/></label
								><label
									>New password<input
										type="password"
										autocomplete="new-password"
										bind:value={newPassword}
										minlength="12"
										maxlength="256"
										required
									/></label
								>
								<p class="muted small-text">Changing your password signs out all sessions.</p>
								<button class="button" disabled={busy}><LockKeyhole size={15} /> Update password</button>
							</form>
						</section>
						<section class="panel settings-panel">
							<h2>System information</h2>
							<dl>
								<dt>Version</dt>
								<dd>{String(system.version || '0.1.0')}</dd>
								<dt>Application URL</dt>
								<dd>{String(system.app_url || '—')}</dd>
								<dt>History retention</dt>
								<dd>90 days</dd>
								<dt>Private network targets</dt>
								<dd>{system.private_targets ? 'Explicitly enabled' : 'Blocked by default'}</dd>
								<dt>Scheduler</dt>
								<dd>One active instance</dd>
								<dt>Display timezone</dt>
								<dd>{Intl.DateTimeFormat().resolvedOptions().timeZone}</dd>
							</dl>
							<div class="notice">
								Brand name and URL are configured through APP_NAME and APP_URL. Accent is configurable per
								public page. Database timestamps are UTC.
							</div>
							<h3>State definitions</h3>
							<p class="muted small-text">
								UP: threshold confirmed healthy. DOWN: failure threshold reached. PENDING: collecting
								evidence. MAINTENANCE: scheduled work. PAUSED: disabled. UNKNOWN: no fresh evidence.
								Historical gaps are excluded from availability.
							</p>
						</section>
						{#if user.role === 'owner'}<section class="panel settings-panel full-grid">
								<h2>Workspace members</h2>
								<div class="member-list">
									{#each members as m}<div class="row-between">
											<span>{m.email} <span class="tag">{m.role}</span></span>{#if m.role !== 'owner'}<button
													class="icon-button danger"
													aria-label={'Remove ' + m.email}
													onclick={() =>
														confirm('Remove this member and revoke all their sessions?', async () => {
															await api('/users/' + m.id, 'DELETE');
															members = await api('/users');
														})}><Trash2 size={15} /></button
												>{/if}
										</div>{/each}
								</div>
								<form
									class="member-form"
									onsubmit={(e) => {
										e.preventDefault();
										void action(async () => {
											await api('/users', 'POST', {
												email: memberEmail,
												password: memberPassword,
												role: memberRole
											});
											members = await api('/users');
											memberPassword = '';
											memberEmail = '';
										}, 'Member created');
									}}
								>
									<label>Email<input type="email" required bind:value={memberEmail} /></label><label
										>Initial password<input
											type="password"
											minlength="12"
											maxlength="256"
											required
											bind:value={memberPassword}
										/></label
									><label
										>Role<select bind:value={memberRole}
											><option value="viewer">Viewer</option><option value="admin">Admin</option></select
										></label
									><button class="button primary" disabled={busy}>Add member</button>
								</form>
							</section>{/if}
					</div>
				{:else}<div class="empty-state">
						<div class="empty-icon"><Search size={32} /></div>
						<h1>That signal is missing.</h1>
						<p>This page or monitor doesn’t exist, or it was removed.</p>
						<a href="/" class="button primary">Back to overview</a>
					</div>{/if}
				<footer class="app-footer">
					<span
						><ShieldCheck size={13} /> {brand} <span class="subtle">/</span> Crafted for peace of mind.</span
					><span><span class="live-dot"></span> Your data. Your infrastructure.</span>
				</footer>
			</main>
		</div>
	</div>
	<Modal
		bind:open={addOpen}
		title={editing?.id ? 'Edit monitor' : 'Add a monitor'}
		description="A meaningful signal starts with a well-defined check."
		>{#if addOpen}<MonitorForm
				monitor={editing}
				{servers}
				onsave={() => void saved()}
				onmessage={notify}
			/>{/if}</Modal
	>
	<Modal
		bind:open={serverOpen}
		title={enrollment ? 'Connect your agent' : 'Enroll a server'}
		description="Outbound-only connections. Independently revocable credentials."
		>{#if enrollment}<div class="notice">
				This token is shown once. Store it in a root-owned environment file, mode 0600, readable by your
				agent’s service account. Rotating it invalidates the old token.
			</div>
			<pre class="secret">AEGIS_URL={typeof location !== 'undefined' ? location.origin : ''}
AEGIS_AGENT_TOKEN={enrollment.token}
AEGIS_DOCKER=false
AEGIS_PM2_HOMES=/home/you/.pm2</pre>
			<button
				class="button"
				onclick={() =>
					void copy(
						`AEGIS_URL=${location.origin}\nAEGIS_AGENT_TOKEN=${enrollment!.token}\nAEGIS_DOCKER=false\nAEGIS_PM2_HOMES=/home/you/.pm2`
					)}><Copy size={15} /> Copy environment</button
			>
			<div class="setup-steps">
				<h3>1. Build and install</h3>
				<code
					>cargo build --release -p aegis-agent<br />sudo install target/release/aegis-agent /usr/local/bin/</code
				>
				<h3>2. Configure the service</h3>
				<p>
					Use deploy/aegis-agent.service and deploy/agent.env.example. Run as the PM2 app owner. For HTTP
					development, set ALLOW_INSECURE_HTTP=true.
				</p>
				<h3>3. Discover and monitor</h3>
				<p>
					The server appears online after its first heartbeat. Choose individual discovered services, and add
					a host monitor for missed-heartbeat alerts.
				</p>
				<div class="error">
					Docker socket access is effectively root access. Enable it only for a trusted local agent. Never
					expose the socket on public TCP.
				</div>
			</div>{:else}<form class="form-stack" onsubmit={addServer}>
				<label
					>Server name<input
						bind:value={serverName}
						required
						maxlength="120"
						placeholder="hetzner-fsn-01"
					/></label
				>{#if formError}<div class="error">{formError}</div>{/if}<button
					class="button primary"
					disabled={busy}>Generate agent credentials <ArrowRight size={16} /></button
				>
			</form>{/if}</Modal
	>
	<Modal
		bind:open={
			() => !!confirmation,
			(v) => {
				if (!v) confirmation = null;
			}
		}
		title="Confirm action"
		description={confirmation?.message || ''}
		>{#if confirmation}<div class="form-actions">
				<button class="button" onclick={() => (confirmation = null)}>Cancel</button><button
					class="button destructive"
					disabled={busy}
					onclick={() =>
						void action(async () => {
							await confirmation?.action();
							confirmation = null;
						}, 'Done')}>Confirm</button
				>
			</div>{/if}</Modal
	>
{/if}
{#if toast}<div class="toast" role="status">
		<Activity size={17} />{toast}<button class="icon-button" aria-label="Dismiss" onclick={() => (toast = '')}
			><X size={15} /></button
		>
	</div>{/if}
