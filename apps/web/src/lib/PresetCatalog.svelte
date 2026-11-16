<script lang="ts">
	import { ArrowUpRight, Check, ExternalLink, RefreshCw, Search, SquareStack } from '@lucide/svelte';
	import { api, type Monitor, type Page, type StatusResource } from './api';
	import { presets, type Preset, type PresetSummary } from './presets';
	let {
		readonly = false,
		onmessage,
		oncreated
	}: { readonly?: boolean; onmessage: (message: string) => void; oncreated: () => Promise<void> } = $props();
	let selected = $state<Preset | null>(null);
	let summary = $state<PresetSummary | null>(null);
	let chosen = $state<string[]>([]);
	let search = $state('');
	let componentSearch = $state('');
	let loading = $state(false);
	let busy = $state(false);
	let error = $state('');
	let existing = $state<Pick<Monitor, 'id' | 'target' | 'config' | 'public'>[]>([]);
	let createdPage = $state('');
	let visible = $derived(
		presets.filter((p) => `${p.name} ${p.category}`.toLowerCase().includes(search.toLowerCase()))
	);
	let components = $derived(
		(summary?.components || []).filter(
			(c) => !c.group && c.name.toLowerCase().includes(componentSearch.toLowerCase())
		)
	);
	let groups = $derived(summary?.components.filter((c) => c.group) || []);
	function groupName(groupId: string | null) {
		return groups.find((g) => g.id === groupId)?.name || 'Services';
	}
	function targetFor(provider: Preset) {
		return provider.id === 'google-cloud'
			? `${provider.origin}/incidents.json`
			: `${provider.origin}/api/v2/summary.json`;
	}
	function configFor(provider: Preset, id: string) {
		return provider.id === 'google-cloud'
			? { status: 200, json_google_product_id: id, check_retries: 1, retention_days: 90 }
			: { status: 200, json_component_id: id, check_retries: 1, retention_days: 90 };
	}
	function monitorFor(id: string) {
		return existing.find(
			(m) =>
				selected &&
				m.target === targetFor(selected) &&
				(m.config?.json_component_id === id || m.config?.json_google_product_id === id)
		);
	}
	async function open(p: Preset) {
		selected = p;
		summary = null;
		chosen = [];
		componentSearch = '';
		createdPage = '';
		await reload();
	}
	async function reload() {
		if (!selected) return;
		loading = true;
		error = '';
		try {
			const [r, monitors] = await Promise.all([
				fetch(`/provider-status/${selected.id}`, { cache: 'no-store' }),
				api<Monitor[]>('/monitors?limit=500')
			]);
			const data = await r.json();
			if (!r.ok) throw new Error(data.error || 'Unable to load provider status');
			summary = data as PresetSummary;
			existing = monitors;
		} catch (e) {
			error = (e as Error).message;
		} finally {
			loading = false;
		}
	}
	function toggle(id: string) {
		chosen = chosen.includes(id) ? chosen.filter((v) => v !== id) : [...chosen, id];
	}
	async function create(publish: boolean) {
		if (!selected || !summary || !chosen.length) return;
		busy = true;
		error = '';
		let count = 0;
		try {
			const resourceIds: string[] = [];
			const resources = publish ? await api<StatusResource[]>('/resources') : [];
			for (const id of chosen) {
				const component = summary.components.find((c) => c.id === id);
				if (!component) continue;
				let monitor = monitorFor(id);
				if (!monitor) {
					const result = await api<{ id: string }>('/monitors', 'POST', {
						name: `${selected.name} · ${component.name}`,
						kind: 'json',
						target: targetFor(selected),
						interval_seconds: 120,
						timeout_seconds: 10,
						failure_threshold: 2,
						recovery_threshold: 2,
						group_name: selected.name,
						description: `Official ${selected.name} status component: ${component.name}`,
						tags: ['preset', selected.id],
						public: publish,
						server_id: null,
						config: configFor(selected, id)
					});
					monitor = {
						id: result.id,
						target: targetFor(selected),
						config: configFor(selected, id),
						public: publish
					};
					existing = [...existing, monitor];
					count++;
				} else if (publish && !monitor.public) {
					throw new Error(
						`${component.name} already has a private monitor. Make it public eligible in Monitors before publishing.`
					);
				}
				if (publish) {
					let resource = resources.find((r) => r.monitor_ids.includes(monitor!.id) && r.public);
					if (!resource) {
						const result = await api<{ id: string }>('/resources', 'POST', {
							name: component.name,
							description: component.description || '',
							group_name: groupName(component.group_id),
							monitor_ids: [monitor!.id],
							public: true,
							override_state: null,
							override_reason: '',
							override_until: null
						});
						resource = { id: result.id } as StatusResource;
					}
					resourceIds.push(resource.id);
				}
			}
			if (publish) {
				const pages = await api<Page[]>('/status-pages');
				let slug = `${selected.id}-status`;
				if (pages.some((p) => p.slug === slug)) slug = `${slug}-${Date.now().toString(36)}`;
				await api('/status-pages', 'POST', {
					slug,
					title: `${selected.name} status`,
					description: `Status of selected ${selected.name} components, observed by Aegis from the provider's public API.`,
					accent: selected.color,
					theme: 'dark',
					layout: 'provider',
					metrics_settings: {
						show_uptime: true,
						show_latency: true,
						show_on_overview: true,
						uptime_days: [1, 7, 30, 90],
						latency_samples: 120
					},
					logo_url: '',
					support_url: selected.origin,
					announcement: '',
					resource_ids: resourceIds,
					monitor_ids: [],
					tabs: [
						{ id: 'overview', label: 'Overview', kind: 'overview' },
						{ id: 'incidents', label: 'Incidents', kind: 'incidents' },
						{ id: 'metrics', label: 'Metrics', kind: 'metrics' }
					]
				});
				createdPage = `/status/${slug}`;
			}
			await oncreated();
			onmessage(
				publish
					? `Created ${selected.name} status page`
					: `Created ${count} component monitor${count === 1 ? '' : 's'}`
			);
		} catch (e) {
			error = (e as Error).message;
		} finally {
			busy = false;
		}
	}
</script>

<div class="preset-page">
	<div class="preset-heading">
		<div>
			<div class="eyebrow">PREMADE MONITORS / PROVIDER STATUS</div>
			<h1>Watch the services behind yours.</h1>
			<p>
				Explore official status components, then turn the ones you rely on into Aegis monitors or a public
				status page.
			</p>
		</div>
		<span class="preset-index">{String(presets.length).padStart(2, '0')}<br />LIVE PROVIDERS</span>
	</div>
	{#if !selected}
		<div class="preset-search">
			<Search size={17} /><input
				aria-label="Search providers"
				placeholder="Find a provider"
				bind:value={search}
			/>
		</div>
		<div class="preset-grid">
			{#each visible as p}<button
					class="preset-card"
					onclick={() => void open(p)}
					style={`--provider:${p.color}`}
					><span class="preset-icon">{p.name.slice(0, 1)}</span><span class="preset-card-main"
						><span class="preset-category">{p.category} / PUBLIC API</span><strong>{p.name}</strong><small
							>Live status · all published components</small
						></span
					><ArrowUpRight size={18} /></button
				>{/each}
		</div>
		<div class="preset-coming">
			<div>
				<span class="preset-category">OTHER PROVIDERS</span>
				<h2>Different APIs, different access.</h2>
				<p>
					Microsoft 365 uses tenant-authorized Graph service health. Google Cloud's public incident feed is
					available above; project-specific Service Health needs Google Cloud authorization. Hetzner publishes
					a status site and feed, but no documented component JSON API.
				</p>
			</div>
			<div class="preset-outlinks">
				<a href="https://status.office.com" target="_blank" rel="noreferrer"
					>Microsoft 365 <ExternalLink size={13} /></a
				><a href="https://status.cloud.google.com" target="_blank" rel="noreferrer"
					>Google Cloud <ExternalLink size={13} /></a
				><a href="https://status.hetzner.com" target="_blank" rel="noreferrer"
					>Hetzner <ExternalLink size={13} /></a
				>
			</div>
		</div>
	{:else}
		<div class="preset-topline">
			<button class="button" onclick={() => (selected = null)}>← All providers</button><a
				class="subtle-link"
				href={selected.origin}
				target="_blank"
				rel="noreferrer">Official status page <ExternalLink size={13} /></a
			>
		</div>
		<div class="preset-provider-hero" style={`--provider:${selected.color}`}>
			<span class="preset-icon large">{selected.name.slice(0, 1)}</span>
			<div>
				<div class="preset-category">{selected.category} / OFFICIAL STATUS API</div>
				<h1>{selected.name}</h1>
				<p>{summary?.status.description || (loading ? 'Loading live status…' : 'Live status unavailable')}</p>
			</div>
			<span class:healthy={summary?.status.indicator === 'none'} class="preset-indicator"
				>{summary?.status.indicator === 'none'
					? selected.id === 'google-cloud'
						? 'No public incident'
						: 'Operational'
					: summary?.status.indicator || 'Unknown'}</span
			>
		</div>
		{#if error}<div class="notice">{error}</div>{/if}
		{#if summary}<div class="preset-controls">
				<div>
					<h2>Published components <span class="muted">{components.length}</span></h2>
					<p class="muted">
						{selected.id === 'google-cloud'
							? 'Based on active public incidents, not project-specific Service Health. Monitor checks poll the official incident feed every two minutes.'
							: `Statuses come from ${selected.name}. Monitor checks poll the official JSON API every two minutes.`}
					</p>
				</div>
				<button class="button small" onclick={() => void reload()} disabled={loading}
					><RefreshCw size={13} /> Refresh</button
				>
			</div>
			<div class="preset-toolbar">
				<div class="preset-search">
					<Search size={15} /><input
						aria-label="Search components"
						placeholder="Filter components"
						bind:value={componentSearch}
					/>
				</div>
				{#if !readonly}<button class="button small" onclick={() => (chosen = components.map((c) => c.id))}
						>Select all shown</button
					><button class="button small" onclick={() => (chosen = [])}>Clear</button>{/if}
			</div>
			<div class="preset-components">
				{#each components as component}<label class="preset-component"
						><span class="preset-component-left"
							>{#if !readonly}<input
									type="checkbox"
									checked={chosen.includes(component.id)}
									onchange={() => toggle(component.id)}
								/>{/if}<span
								><strong>{component.name}</strong><small
									>{groupName(component.group_id)}{monitorFor(component.id)
										? ' · Monitoring in Aegis'
										: ''}</small
								></span
							></span
						><span class:good={component.status === 'operational'} class="preset-state"
							><i></i>{selected.id === 'google-cloud' && component.status === 'operational'
								? 'No public incident'
								: component.status.replaceAll('_', ' ')}</span
						></label
					>{/each}
			</div>
			{#if !readonly}<div class="preset-actions">
					<span>{chosen.length} selected</span><button
						class="button"
						disabled={busy || !chosen.length}
						onclick={() => void create(false)}><Check size={15} /> Create monitors</button
					><button class="button primary" disabled={busy || !chosen.length} onclick={() => void create(true)}
						><SquareStack size={15} /> Create public status page</button
					>
				</div>{/if}
			{#if createdPage}<div class="notice">
					<a href={createdPage} target="_blank" rel="noreferrer">Open new status page ↗</a>
				</div>{/if}
			{#if summary.incidents.length}<div class="preset-incidents">
					<h2>Active provider incidents</h2>
					{#each summary.incidents as incident}<div>
							<strong>{incident.name}</strong><span>{incident.status.replaceAll('_', ' ')}</span>
						</div>{/each}
				</div>{/if}
		{/if}
	{/if}
</div>

<style>
	.preset-page {
		max-width: 1160px;
		margin: auto;
		padding-bottom: 40px;
	}
	.preset-heading {
		display: flex;
		justify-content: space-between;
		align-items: end;
		margin-bottom: 27px;
	}
	.preset-heading h1 {
		font-size: 38px;
		margin: 8px 0 9px;
		letter-spacing: -1.6px;
	}
	.preset-heading p,
	.preset-coming p {
		color: var(--muted);
		max-width: 680px;
	}
	.preset-index {
		font:
			11px/1.5 Consolas,
			monospace;
		text-align: right;
		color: var(--muted);
		letter-spacing: 1px;
	}
	.preset-search {
		display: flex;
		align-items: center;
		gap: 10px;
		border: 1px solid var(--border);
		background: #111219;
		padding: 0 13px;
		border-radius: 8px;
		min-width: 220px;
	}
	.preset-search input {
		border: 0;
		background: none;
		box-shadow: none;
		padding: 12px 0;
	}
	.preset-grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 12px;
		margin: 20px 0 30px;
	}
	.preset-card {
		text-align: left;
		display: flex;
		align-items: center;
		gap: 18px;
		border: 1px solid var(--border);
		background: linear-gradient(120deg, #171820, #101116);
		padding: 22px;
		border-radius: 12px;
		min-height: 120px;
		transition:
			transform 0.16s,
			border-color 0.16s;
	}
	.preset-card:hover {
		transform: translateY(-3px);
		border-color: var(--provider);
	}
	.preset-icon {
		display: grid;
		place-items: center;
		width: 48px;
		height: 48px;
		border-radius: 12px;
		background: color-mix(in srgb, var(--provider) 18%, transparent);
		color: var(--provider);
		font-size: 25px;
		font-weight: 750;
		flex: none;
	}
	.preset-icon.large {
		width: 65px;
		height: 65px;
		font-size: 34px;
	}
	.preset-card-main {
		display: flex;
		flex-direction: column;
		gap: 2px;
		flex: 1;
	}
	.preset-card-main strong {
		font-size: 19px;
	}
	.preset-card-main small,
	.preset-category {
		font-size: 10px;
		letter-spacing: 1px;
		color: var(--muted);
		text-transform: uppercase;
	}
	.preset-coming {
		border: 1px solid var(--border);
		border-radius: 12px;
		padding: 25px;
		display: flex;
		justify-content: space-between;
		gap: 30px;
		background: #101116;
	}
	.preset-coming h2 {
		font-size: 18px;
		margin: 7px 0;
	}
	.preset-outlinks {
		display: flex;
		flex-direction: column;
		gap: 8px;
		min-width: 170px;
	}
	.preset-outlinks a {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		color: #c7bbff;
	}
	.preset-topline {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 18px;
	}
	.preset-provider-hero {
		border: 1px solid color-mix(in srgb, var(--provider) 36%, var(--border));
		border-radius: 14px;
		background:
			radial-gradient(
				circle at 85% 10%,
				color-mix(in srgb, var(--provider) 13%, transparent),
				transparent 40%
			),
			#14151b;
		padding: 25px;
		display: flex;
		align-items: center;
		gap: 20px;
	}
	.preset-provider-hero h1 {
		margin: 5px 0;
	}
	.preset-provider-hero p {
		color: var(--muted);
	}
	.preset-indicator {
		margin-left: auto;
		text-transform: capitalize;
		padding: 7px 12px;
		border-radius: 30px;
		background: #55303b;
		color: #ffb4c4;
		white-space: nowrap;
	}
	.preset-indicator.healthy {
		background: #1e4538;
		color: #8ce3bd;
	}
	.preset-controls,
	.preset-toolbar,
	.preset-actions {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 12px;
		margin: 24px 0 12px;
	}
	.preset-controls p {
		margin-top: 4px;
	}
	.preset-toolbar {
		justify-content: flex-start;
	}
	.preset-toolbar .preset-search {
		flex: 1;
	}
	.preset-components {
		border: 1px solid var(--border);
		border-radius: 10px;
		overflow: hidden;
	}
	.preset-component {
		display: flex;
		flex-direction: row;
		justify-content: space-between;
		align-items: center;
		gap: 15px;
		padding: 13px 17px;
		border-bottom: 1px solid var(--border);
		background: #111218;
	}
	.preset-component:last-child {
		border-bottom: 0;
	}
	.preset-component:hover {
		background: #171820;
	}
	.preset-component-left {
		display: flex;
		align-items: center;
		gap: 12px;
	}
	.preset-component-left strong {
		display: block;
		font-size: 13px;
	}
	.preset-component-left small {
		display: block;
		color: var(--muted);
		font-size: 11px;
		margin-top: 2px;
	}
	.preset-state {
		display: flex;
		align-items: center;
		gap: 7px;
		color: #f4aaaf;
		text-transform: capitalize;
		white-space: nowrap;
		font-size: 11px;
	}
	.preset-state.good {
		color: var(--green);
	}
	.preset-state i {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: currentColor;
	}
	.preset-actions {
		justify-content: flex-end;
		position: sticky;
		bottom: 12px;
		border: 1px solid var(--border);
		background: #1b1c26f5;
		padding: 12px;
		border-radius: 10px;
		z-index: 2;
	}
	.preset-actions span {
		margin-right: auto;
		color: var(--muted);
	}
	.preset-incidents {
		margin-top: 28px;
		border: 1px solid var(--border);
		border-radius: 10px;
		padding: 18px;
	}
	.preset-incidents h2 {
		margin-bottom: 10px;
	}
	.preset-incidents > div {
		display: flex;
		justify-content: space-between;
		padding: 8px 0;
		border-top: 1px solid var(--border);
	}
	.preset-incidents span {
		text-transform: capitalize;
		color: var(--muted);
	}
	@media (max-width: 720px) {
		.preset-grid {
			grid-template-columns: 1fr;
		}
		.preset-heading h1 {
			font-size: 29px;
		}
		.preset-index {
			display: none;
		}
		.preset-coming {
			flex-direction: column;
		}
		.preset-provider-hero {
			flex-wrap: wrap;
		}
		.preset-indicator {
			margin-left: 0;
		}
		.preset-actions {
			flex-wrap: wrap;
		}
		.preset-actions span {
			width: 100%;
		}
	}
</style>
