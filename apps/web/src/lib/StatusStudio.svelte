<script lang="ts">
	import { customSelect } from './customSelect';
	import { onMount } from 'svelte';
	import {
		Plus,
		Globe,
		Layers,
		Pencil,
		Trash2,
		ExternalLink,
		ArrowUp,
		ArrowDown,
		ShieldCheck,
		Clock
	} from '@lucide/svelte';
	import {
		api,
		time,
		defaultTabs,
		defaultMetricsSettings,
		resourceStates,
		type Page,
		type StatusResource,
		type StatusTab,
		type Monitor,
		type ResourceState,
		type StatusMetricsSettings
	} from './api';
	import Modal from './Modal.svelte';
	let { readonly = false, onmessage }: { readonly?: boolean; onmessage: (s: string) => void } = $props();
	let pages = $state<Page[]>([]);
	let resources = $state<StatusResource[]>([]);
	let monitors = $state<Monitor[]>([]);
	let section = $state('pages');
	let error = $state('');
	let busy = $state(false);
	let loading = $state(true);
	let resourceOpen = $state(false);
	let pageOpen = $state(false);
	let resourceId = $state<string | null>(null);
	let pageId = $state<string | null>(null);
	let resourceName = $state('');
	let resourceDescription = $state('');
	let group = $state('');
	let linked = $state<string[]>([]);
	let resourcePublic = $state(false);
	let override = $state('automatic');
	let overrideReason = $state('');
	let until = $state('');
	let title = $state('');
	let slug = $state('');
	let description = $state('');
	let accent = $state('#9a85ff');
	let theme = $state('dark');
	let layout = $state<'standard' | 'provider'>('standard');
	let metricsSettings = $state<StatusMetricsSettings>({
		...defaultMetricsSettings,
		uptime_days: [...defaultMetricsSettings.uptime_days]
	});
	let logo = $state('');
	let support = $state('');
	let supportLabel = $state('Contact support');
	let footerText = $state('');
	let subscriptionsEnabled = $state(true);
	let announcement = $state('');
	let selectedResources = $state<string[]>([]);
	let selectedMonitors = $state<string[]>([]);
	let tabs = $state<StatusTab[]>(defaultTabs.map((tab) => ({ ...tab })));
	let previewTab = $state('overview');
	async function load() {
		try {
			[pages, resources, monitors] = await Promise.all([
				api<Page[]>('/status-pages'),
				api<StatusResource[]>('/resources'),
				api<Monitor[]>('/monitors')
			]);
		} catch (e) {
			error = (e as Error).message;
		} finally {
			loading = false;
		}
	}
	onMount(() => {
		void load();
	});
	function editResource(r?: StatusResource) {
		resourceId = r?.id || null;
		resourceName = r?.name || '';
		resourceDescription = r?.description || '';
		group = r?.group_name || '';
		linked = [...(r?.monitor_ids || [])];
		resourcePublic = r?.public || false;
		override = r?.override_state || 'automatic';
		overrideReason = r?.override_reason || '';
		until = r?.override_until ? localDate(r.override_until) : '';
		error = '';
		resourceOpen = true;
	}
	function localDate(value: string) {
		const d = new Date(value);
		return new Date(d.getTime() - d.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
	}
	function editPage(p?: Page) {
		pageId = p?.id || null;
		title = p?.title || '';
		slug = p?.slug || '';
		description = p?.description || '';
		accent = p?.accent || '#9a85ff';
		theme = p?.theme || 'dark';
		layout = p?.layout || 'standard';
		metricsSettings = {
			...defaultMetricsSettings,
			...p?.metrics_settings,
			uptime_days: [...(p?.metrics_settings?.uptime_days || defaultMetricsSettings.uptime_days)]
		};
		logo = p?.logo_url || '';
		support = p?.support_url || '';
		supportLabel = p?.support_label || 'Contact support';
		footerText = p?.footer_text || '';
		subscriptionsEnabled = p?.subscriptions_enabled ?? true;
		announcement = p?.announcement || '';
		selectedResources = [...(p?.resource_ids || [])];
		selectedMonitors = [...(p?.monitor_ids || [])];
		tabs = (p?.tabs || defaultTabs).map((tab) => ({ ...tab }));
		previewTab = tabs[0]?.id || 'overview';
		error = '';
		pageOpen = true;
	}
	async function saveResource(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = '';
		try {
			await api('/resources' + (resourceId ? '/' + resourceId : ''), resourceId ? 'PUT' : 'POST', {
				name: resourceName,
				description: resourceDescription,
				group_name: group,
				monitor_ids: linked,
				public: resourcePublic,
				override_state: override === 'automatic' ? null : override,
				override_reason: override === 'automatic' ? '' : overrideReason,
				override_until: override !== 'automatic' && until ? new Date(until).toISOString() : null
			});
			resourceOpen = false;
			await load();
			onmessage('Resource saved. Manual status changes are audited separately from check history.');
		} catch (e) {
			error = (e as Error).message;
		} finally {
			busy = false;
		}
	}
	async function savePage(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = '';
		try {
			await api('/status-pages' + (pageId ? '/' + pageId : ''), pageId ? 'PUT' : 'POST', {
				slug,
				title,
				description,
				accent,
				theme,
				layout,
				metrics_settings: metricsSettings,
				logo_url: logo,
				support_url: support,
				support_label: supportLabel,
				footer_text: footerText,
				subscriptions_enabled: subscriptionsEnabled,
				announcement,
				resource_ids: selectedResources,
				monitor_ids: selectedMonitors,
				tabs
			});
			pageOpen = false;
			await load();
			onmessage('Status page saved');
		} catch (e) {
			error = (e as Error).message;
		} finally {
			busy = false;
		}
	}
	async function remove(path: string, label: string) {
		if (!confirm(`Permanently delete ${label}?`)) return;
		try {
			await api(path, 'DELETE');
			await load();
			onmessage('Deleted');
		} catch (e) {
			onmessage((e as Error).message);
		}
	}
	function moveResource(id: string, direction: number) {
		const ids = [...selectedResources];
		const i = ids.indexOf(id);
		const j = i + direction;
		if (j < 0 || j >= ids.length) return;
		[ids[i], ids[j]] = [ids[j], ids[i]];
		selectedResources = ids;
	}
	function moveTab(index: number, direction: number) {
		const next = [...tabs];
		const j = index + direction;
		if (j < 0 || j >= next.length) return;
		[next[index], next[j]] = [next[j], next[index]];
		tabs = next;
	}
	function label(r: StatusResource) {
		if (r.override_state && (!r.override_until || new Date(r.override_until).getTime() > Date.now()))
			return resourceStates.find((s) => s.value === r.override_state)?.label || 'Unknown';
		return r.monitor_ids.length ? 'Automatic' : 'Unknown — no observations';
	}
	let publicResources = $derived(resources.filter((r) => r.public));
	function pageMembers(p: Page) {
		return [
			...(p.resource_ids || []).map((id) => resources.find((r) => r.id === id)?.name || 'Removed resource'),
			...(p.monitor_ids || []).map((id) => monitors.find((m) => m.id === id)?.name || 'Removed monitor')
		];
	}
</script>

<div class="page-heading">
	<div>
		<div class="eyebrow">TRANSPARENCY BUILDS TRUST</div>
		<h1>Share the bigger picture.</h1>
		<p class="muted">
			Build public resources, curate tabs, and communicate changes without exposing private internals.
		</p>
	</div>
	{#if !readonly}<button
			class="button primary"
			onclick={() => (section === 'resources' ? editResource() : editPage())}
			><Plus size={16} />{section === 'resources' ? 'Create resource' : 'Create status page'}</button
		>{/if}
</div>
{#if !loading}<div class="studio-summary" aria-label="Status page overview">
		<div>
			<Globe size={18} /><span
				><strong>{pages.length}</strong><small>Status {pages.length === 1 ? 'page' : 'pages'}</small></span
			>
		</div>
		<div>
			<Layers size={18} /><span><strong>{publicResources.length}</strong><small>Public resources</small></span
			>
		</div>
		<div>
			<ShieldCheck size={18} /><span
				><strong>{monitors.filter((m) => m.public).length}</strong><small>Eligible monitors</small></span
			>
		</div>
	</div>{/if}
<div class="studio-tabs" role="tablist" aria-label="Status studio sections">
	<button
		role="tab"
		aria-selected={section === 'pages'}
		class:chosen={section === 'pages'}
		onclick={() => (section = 'pages')}><Globe size={15} /> Status pages <span>{pages.length}</span></button
	><button
		role="tab"
		aria-selected={section === 'resources'}
		class:chosen={section === 'resources'}
		onclick={() => (section = 'resources')}
		><Layers size={15} /> Resources <span>{resources.length}</span></button
	>
</div>
{#if error && !pageOpen && !resourceOpen}<div class="error" role="alert">{error}</div>{/if}{#if loading}<div
		class="panel calm-state"
	>
		Loading status studio…
	</div>{:else if section === 'pages'}<div class="channel-grid">
		{#each pages as p}<section class="panel status-page-card">
				<div class="status-preview" class:light={p.theme === 'light'} style={`--page-accent:${p.accent}`}>
					<div class="preview-top">
						<span class="preview-mark"><ShieldCheck size={22} /></span><span class="preview-url"
							>/status/{p.slug}</span
						>
					</div>
					<h3>{p.title}</h3>
					<p class="preview-description">{p.description || 'Live service health and updates'}</p>
					<div class="preview-service">
						<span class="preview-service-mark"></span>{pageMembers(p)[0] || 'Service overview'}<span
							>● ● ● ● ●</span
						>
					</div>
					<div class="page-tab-preview">
						{#each p.tabs || defaultTabs as tab}<span>{tab.label}</span>{/each}
					</div>
				</div>
				<div class="status-page-info">
					<div class="row-between">
						<h2>{p.title}</h2>
						<span class="pill green">Published</span>
					</div>
					<p class="muted">
						{p.resource_ids?.length || 0} resources · {p.monitor_ids?.length || 0} direct monitors · {(
							p.tabs || defaultTabs
						).length} tabs
					</p>
					<div class="page-members">
						{#each pageMembers(p).slice(0, 3) as member}<span>{member}</span>{:else}<span class="empty-member"
								>No services selected</span
							>{/each}{#if pageMembers(p).length > 3}<span>+{pageMembers(p).length - 3} more</span>{/if}
					</div>
					<div class="row-between">
						<a class="button small" href={'/status/' + p.slug} target="_blank" rel="noreferrer"
							>Open page <ExternalLink size={13} /></a
						>{#if !readonly}<div class="button-group">
								<button class="button small" onclick={() => editPage(p)}><Pencil size={12} /> Edit</button
								><button
									class="icon-button danger"
									aria-label={'Delete ' + p.title}
									onclick={() => void remove('/status-pages/' + p.id, p.title)}><Trash2 size={15} /></button
								>
							</div>{/if}
					</div>
				</div>
			</section>{:else}<section class="panel empty-state full-grid">
				<div class="empty-icon"><Globe size={30} /></div>
				<h3>Your services deserve a clear public story.</h3>
				<p>
					Create a page, select public resources or monitors,<br />then add incident, maintenance, metrics and
					custom text tabs.
				</p>
				{#if !readonly}<button class="button primary" onclick={() => editPage()}>Create a status page</button
					>{/if}
			</section>{/each}
	</div>
{:else}<div class="notice studio-notice">
		Resources are public-facing services such as “Payments” or “Bot platform”. Link one or more
		public-eligible monitors, or use an explicitly manual state. Overrides never change measured uptime.
	</div>
	<div class="channel-grid">
		{#each resources as r}<section class="panel resource-card">
				<div class="row-between">
					<span class="tag">{r.group_name || 'Ungrouped'}</span><span class="pill"
						>{r.public ? 'Public eligible' : 'Private'}</span
					>
				</div>
				<h2>{r.name}</h2>
				<p class="muted">{r.description || 'No public description.'}</p>
				<div class="resource-status-label {r.override_state || ''}">
					<span class="status-dot"></span>{label(r)}
				</div>
				{#if r.override_state}<p class="small-text muted">{r.override_reason}</p>
					{#if r.override_until}<p class="small-text muted">
							<Clock size={12} /> Override until {time(r.override_until)}
						</p>{/if}{/if}
				<div class="row-between">
					<span class="small-text muted">{r.monitor_ids.length} linked monitors</span>{#if !readonly}<div
							class="button-group"
						>
							<button class="button small" onclick={() => editResource(r)}
								><Pencil size={12} /> Edit status</button
							><button
								class="icon-button danger"
								aria-label={'Delete ' + r.name}
								onclick={() => void remove('/resources/' + r.id, r.name)}><Trash2 size={14} /></button
							>
						</div>{/if}
				</div>
			</section>{:else}<section class="panel empty-state full-grid">
				<div class="empty-icon"><Layers size={29} /></div>
				<h3>Make infrastructure understandable.</h3>
				<p>
					Group technical monitors into services your users recognize.<br />Automatic health and editorial
					overrides stay distinct.
				</p>
				{#if !readonly}<button class="button primary" onclick={() => editResource()}
						>Create your first resource</button
					>{/if}
			</section>{/each}
	</div>{/if}
<Modal
	bind:open={resourceOpen}
	title={resourceId ? 'Edit resource and status' : 'Create public resource'}
	description="Public names, descriptions and override reasons are visible when this resource is published."
	><form class="form-stack" onsubmit={saveResource}>
		<div class="form-row">
			<label
				>Resource name<input
					bind:value={resourceName}
					required
					maxlength="120"
					placeholder="Payments API"
				/></label
			><label>Group<input bind:value={group} maxlength="120" placeholder="Core services" /></label>
		</div>
		<label>Public description<textarea bind:value={resourceDescription} maxlength="2000"></textarea></label
		><label class="checkbox"
			><input type="checkbox" bind:checked={resourcePublic} /> Allow this resource on public pages</label
		>
		<fieldset>
			<legend>Linked public-eligible monitors</legend>
			<div class="selection-list">
				{#each monitors.filter((m) => m.public) as m}<label class="checkbox"
						><input type="checkbox" value={m.id} bind:group={linked} />{m.name}
						<span class="muted">{m.kind.toUpperCase()}</span></label
					>{:else}<p class="muted small-text">
						Make a monitor public eligible in its advanced settings first.
					</p>{/each}
			</div>
		</fieldset>
		<label
			>Published resource status<select use:customSelect bind:value={override}
				><option value="automatic">Automatic — derive from linked monitors</option
				>{#each resourceStates as state}<option value={state.value}>{state.label} — manual override</option
					>{/each}</select
			></label
		>{#if override !== 'automatic'}<label
				>Public override reason<textarea
					bind:value={overrideReason}
					required
					maxlength="1000"
					placeholder="We are investigating intermittent delays."
				></textarea></label
			><label>Override expires (optional, local time)<input type="datetime-local" bind:value={until} /></label
			>
			<div class="notice">
				Manual status is labeled on the public page and recorded in the audit log. It never changes checks,
				incidents, or availability calculations.
			</div>{:else}<p class="muted small-text">
				All linked monitors must be healthy for Operational. Missing or private observations result in
				Unknown. A resource with no linked monitors has no measured uptime.
			</p>{/if}{#if error}<div class="error" role="alert">{error}</div>{/if}<button
			class="button primary"
			disabled={busy}>{busy ? 'Saving…' : 'Save resource'}</button
		>
	</form></Modal
>
<Modal
	bind:open={pageOpen}
	title={pageId ? 'Edit status page' : 'Create a status page'}
	description="Choose what visitors see, then open the published page to review it."
	><form class="form-stack" onsubmit={savePage}>
		<section class="status-editor-preview" aria-label="Live status page preview">
			<div class="status-preview" class:light={theme === 'light'} style={`--page-accent:${accent}`}>
				<div class="preview-top">
					<span class="preview-mark"><ShieldCheck size={22} /></span>
					<span class="preview-url">/status/{slug || 'your-page'}</span>
				</div>
				{#if announcement}<div class="status-editor-announcement">{announcement}</div>{/if}
				<h3>{title || 'Your status page'}</h3>
				<p class="preview-description">{description || 'Describe the services on this page.'}</p>
				<div class="page-tab-preview">
					{#each tabs as tab}<button type="button" class:chosen={previewTab === tab.id} onclick={() => (previewTab = tab.id)}>{tab.label || 'Untitled tab'}</button>{/each}
				</div>
				{#if tabs.find((tab) => tab.id === previewTab)?.kind === 'text'}
					<p class="preview-description">{tabs.find((tab) => tab.id === previewTab)?.body || 'Add text to this tab.'}</p>
				{:else}
					{#each selectedResources as id}<div class="preview-service"><span class="preview-service-mark"></span>{resources.find((r) => r.id === id)?.name || 'Removed resource'}</div>{/each}
					{#each selectedMonitors as id}<div class="preview-service"><span class="preview-service-mark"></span>{monitors.find((m) => m.id === id)?.name || 'Removed monitor'}</div>{/each}
				{/if}
				{#if support}<p class="preview-description">{supportLabel} ↗</p>{/if}
				{#if footerText}<p class="preview-description">{footerText}</p>{/if}
			</div>
			<p class="muted small-text">Draft preview of appearance and content. Save and open the page to review live health, incidents, and graphs.</p>
		</section>
		<div class="editor-intro">
			<span>01 / IDENTITY</span>
			<p>Give visitors a recognizable page and a short explanation of what it covers.</p>
		</div>
		<div class="form-row">
			<label
				>Page title<input bind:value={title} required maxlength="120" placeholder="Service status" /></label
			><label
				>Slug<input
					bind:value={slug}
					pattern="(?:[a-z0-9]|-)+"
					required
					maxlength="80"
					placeholder="platform"
				/></label
			>
		</div>
		<label>Description<textarea bind:value={description} maxlength="2000"></textarea></label>
		<div class="form-row">
			<label>Accent color<input type="color" bind:value={accent} /></label><label
				>Theme<select use:customSelect bind:value={theme}
					><option value="dark">Dark</option><option value="light">Light</option><option value="auto"
						>System preference</option
					></select
				></label
			>
		</div>
		<label
			>Page layout<select use:customSelect bind:value={layout}
				><option value="standard">Standard service status</option><option value="provider"
					>Provider components</option
				></select
			></label
		>
		<div class="form-row">
			<label>Logo HTTPS URL<input type="url" bind:value={logo} placeholder="https://…" /></label><label
				>Support HTTPS URL<input type="url" bind:value={support} placeholder="https://…" /></label
			>
		</div>
		<label>Support link label<input bind:value={supportLabel} required maxlength="60" /></label>
		<label>Footer text<textarea bind:value={footerText} maxlength="500" placeholder="Optional public footer note"></textarea></label>
		<label class="checkbox"><input type="checkbox" bind:checked={subscriptionsEnabled} /> Allow visitors to subscribe to published incident updates by email</label>
		<label
			>Announcement banner<textarea
				bind:value={announcement}
				maxlength="1000"
				placeholder="Public announcement, optional"
			></textarea></label
		>
		<div class="editor-intro">
			<span>02 / SERVICES</span>
			<p>Choose the public resources visitors should see and arrange their display order.</p>
		</div>
		<fieldset>
			<legend>Public resources</legend>
			<div class="selection-list">
				{#each resources.filter((r) => r.public) as r}<label class="checkbox"
						><input type="checkbox" value={r.id} bind:group={selectedResources} />{r.name}
						<span class="muted">{r.group_name}</span></label
					>{:else}<p class="muted small-text">
						Create a public-eligible resource in the Resources tab.
					</p>{/each}
			</div>
			{#if selectedResources.length}<div class="ordered-list">
					{#each selectedResources as id, i}<div>
							<span>{i + 1}. {resources.find((r) => r.id === id)?.name || 'Unavailable resource'}</span>
							<div class="button-group">
								<button
									class="icon-button"
									type="button"
									disabled={i === 0}
									aria-label="Move resource up"
									onclick={() => moveResource(id, -1)}><ArrowUp size={13} /></button
								><button
									class="icon-button"
									type="button"
									disabled={i === selectedResources.length - 1}
									aria-label="Move resource down"
									onclick={() => moveResource(id, 1)}><ArrowDown size={13} /></button
								>
							</div>
						</div>{/each}
				</div>{/if}
		</fieldset>
		<details>
			<summary>Direct monitor selection (legacy / simple pages)</summary>
			<div class="selection-list inset">
				{#each monitors.filter((m) => m.public) as m}<label class="checkbox"
						><input type="checkbox" value={m.id} bind:group={selectedMonitors} />{m.name}</label
					>{/each}
			</div>
			<p class="muted small-text">
				Direct monitors keep their measured availability bars. Resource groups present an aggregate health
				state.
			</p>
		</details>
		<div class="editor-intro">
			<span>03 / CONTENT</span>
			<p>Build navigation for incidents, maintenance, uptime, and custom information.</p>
		</div>
		<div class="section-heading">
			<h3>Public page tabs</h3>
			<button
				class="button small"
				type="button"
				disabled={tabs.length >= 8}
				onclick={() =>
					(tabs = [
						...tabs,
						{
							id: 'tab-' + crypto.randomUUID().slice(0, 8),
							label: 'More information',
							kind: 'text',
							body: ''
						}
					])}><Plus size={12} /> Add tab</button
			>
		</div>
		{#each tabs as tab, i}<div class="editor-field">
				<div class="form-row">
					<label>Tab label<input bind:value={tab.label} maxlength="40" required /></label><label
						>Content<select use:customSelect bind:value={tab.kind}
							><option value="overview">Service overview</option><option value="incidents"
								>Incident history</option
							><option value="maintenance">Planned maintenance</option><option value="metrics"
								>Measured uptime</option
							><option value="text">Custom information</option></select
						></label
					>
				</div>
				<label
					>Tab ID (URL hash)<input
						bind:value={tab.id}
						pattern="(?:[a-z0-9]|-)+"
						maxlength="40"
						required
					/></label
				>{#if tab.kind === 'text'}<label
						>Public text<textarea
							bind:value={tab.body}
							maxlength="8000"
							rows="5"
							placeholder="Support details, service-level expectations, or other public information. Plain text only."
						></textarea></label
					>{/if}
				<div class="row-between">
					<span class="muted small-text"
						>{tab.kind === 'text'
							? 'Plain text is escaped for safety.'
							: 'Content comes from real public-safe API data.'}</span
					>
					<div class="button-group">
						<button
							class="icon-button"
							type="button"
							aria-label="Move tab up"
							disabled={i === 0}
							onclick={() => moveTab(i, -1)}><ArrowUp size={13} /></button
						><button
							class="icon-button"
							type="button"
							aria-label="Move tab down"
							disabled={i === tabs.length - 1}
							onclick={() => moveTab(i, 1)}><ArrowDown size={13} /></button
						><button
							type="button"
							class="icon-button danger"
							aria-label="Remove tab"
							disabled={tabs.length <= 1}
							onclick={() => (tabs = tabs.filter((_, j) => j !== i))}><Trash2 size={13} /></button
						>
					</div>
				</div>
			</div>{/each}
		<div class="editor-intro">
			<span>04 / GRAPHS</span>
			<p>Choose the measured charts and ranges for this page. Changes only affect this page.</p>
		</div>
		<fieldset>
			<legend>Public graphs</legend>
			<label class="checkbox"
				><input type="checkbox" bind:checked={metricsSettings.show_uptime} /> Show uptime comparison</label
			>
			<label class="checkbox"
				><input type="checkbox" bind:checked={metricsSettings.show_latency} /> Show response-time graph</label
			>
			<label class="checkbox"
				><input type="checkbox" bind:checked={metricsSettings.show_on_overview} /> Show graphs on overview</label
			>
			<div class="form-row">
				<label
					>Response samples<select use:customSelect bind:value={metricsSettings.latency_samples}
						><option value={60}>Latest 60</option><option value={120}>Latest 120</option><option value={240}
							>Latest 240</option
						></select
					></label
				>
			</div>
			<div class="selection-list">
				<span class="muted small-text">Uptime comparison periods</span
				>{#each [[1, '24 hours'], [7, '7 days'], [30, '30 days'], [90, '90 days']] as [days, label]}<label
						class="checkbox"
						><input
							type="checkbox"
							checked={metricsSettings.uptime_days.includes(Number(days))}
							disabled={metricsSettings.uptime_days.length === 1 &&
								metricsSettings.uptime_days.includes(Number(days))}
							onchange={() =>
								(metricsSettings.uptime_days = metricsSettings.uptime_days.includes(Number(days))
									? metricsSettings.uptime_days.filter((v) => v !== Number(days))
									: [...metricsSettings.uptime_days, Number(days)].sort((a, b) => a - b))}
						/>
						{label}</label
					>{/each}
			</div>
			<p class="muted small-text">
				Uptime is time-weighted from observed checks. Response time is measured probe duration; unavailable
				samples are omitted.
			</p>
		</fieldset>
		{#if error}<div class="error" role="alert">{error}</div>{/if}
		<div class="form-actions">
			<button class="button primary" disabled={busy}>{busy ? 'Saving…' : 'Save status page'}</button>
		</div>
	</form></Modal
>
