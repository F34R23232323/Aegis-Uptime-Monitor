<script lang="ts">
	import { onMount } from 'svelte';
	import {
		Plus,
		Check,
		TriangleAlert,
		Pencil,
		MessageSquare,
		RefreshCw,
		Globe,
		LockKeyhole
	} from '@lucide/svelte';
	import {
		api,
		time,
		duration,
		type Incident,
		type Page,
		type StatusResource,
		type IncidentUpdate
	} from './api';
	import Modal from './Modal.svelte';
	let { readonly = false, onmessage }: { readonly?: boolean; onmessage: (s: string) => void } = $props();
	let incidents = $state<Incident[]>([]);
	let pages = $state<Page[]>([]);
	let resources = $state<StatusResource[]>([]);
	let open = $state(false);
	let updateOpen = $state(false);
	let editing = $state<Incident | null>(null);
	let title = $state('');
	let severity = $state('major');
	let phase = $state('investigating');
	let message = $state('');
	let note = $state('');
	let published = $state(false);
	let selectedPages = $state<string[]>([]);
	let selectedResources = $state<string[]>([]);
	let updates = $state<Record<string, IncidentUpdate[]>>({});
	let error = $state('');
	let busy = $state(false);
	let filter = $state('all');
	async function load() {
		try {
			[incidents, pages, resources] = await Promise.all([
				api<Incident[]>('/incidents'),
				api<Page[]>('/status-pages'),
				api<StatusResource[]>('/resources')
			]);
		} catch (e) {
			error = (e as Error).message;
		}
	}
	onMount(() => {
		void load();
		const id = setInterval(() => void load(), 30000);
		return () => clearInterval(id);
	});
	function edit(i?: Incident) {
		editing = i || null;
		title = i?.title || i?.monitor_name || '';
		severity = i?.severity || 'major';
		phase = i?.phase || (i?.ended_at ? 'resolved' : 'investigating');
		message = i?.public_message || '';
		note = i?.note || '';
		published = i?.published || false;
		selectedPages = i?.status_page_ids || [];
		selectedResources = i?.resource_ids || [];
		error = '';
		open = true;
	}
	async function save(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = '';
		try {
			await api('/incidents' + (editing ? '/' + editing.id : ''), editing ? 'PUT' : 'POST', {
				title,
				severity,
				phase,
				public_message: message,
				published,
				resource_ids: selectedResources,
				status_page_ids: selectedPages,
				note
			});
			open = false;
			await load();
			onmessage('Incident saved');
		} catch (e) {
			error = (e as Error).message;
		} finally {
			busy = false;
		}
	}
	async function timeline(id: string) {
		try {
			updates[id] = await api<IncidentUpdate[]>('/incidents/' + id + '/updates');
		} catch (e) {
			onmessage((e as Error).message);
		}
	}
	function startUpdate(i: Incident) {
		editing = i;
		phase = i.phase || 'investigating';
		message = '';
		error = '';
		updateOpen = true;
	}
	async function postUpdate(e: SubmitEvent) {
		e.preventDefault();
		if (!editing) return;
		busy = true;
		error = '';
		try {
			await api('/incidents/' + editing.id + '/updates', 'POST', { phase, message });
			await timeline(editing.id);
			updateOpen = false;
			await load();
			onmessage('Incident update posted');
		} catch (e) {
			error = (e as Error).message;
		} finally {
			busy = false;
		}
	}
</script>

<div class="page-heading">
	<div>
		<div class="eyebrow">FROM FIRST SIGNAL TO RESOLUTION</div>
		<h1>Incident journal.</h1>
		<p class="muted">Manage measured interruptions and editorial communications in one durable timeline.</p>
	</div>
	{#if !readonly}<button class="button primary" onclick={() => edit()}
			><Plus size={15} /> Create incident</button
		>{/if}
</div>
<div class="section-heading">
	<div class="filter-tabs">
		{#each [['all', 'All incidents'], ['open', 'Open'], ['resolved', 'Resolved'], ['published', 'Published']] as [value, label]}<button
				class:chosen={filter === value}
				onclick={() => (filter = value)}>{label}</button
			>{/each}
	</div>
	<button class="icon-button" aria-label="Refresh incidents" onclick={() => void load()}
		><RefreshCw size={15} /></button
	>
</div>
{#if error && !open && !updateOpen}<div class="error" role="alert">{error}</div>{/if}
<section class="panel">
	{#each incidents.filter((i) => filter === 'all' || (filter === 'open' && !i.ended_at) || (filter === 'resolved' && !!i.ended_at) || (filter === 'published' && i.published)) as i}<article
			class="incident-entry"
		>
			<div class="incident-line">
				<span class:resolved={!!i.ended_at} class="event-icon"
					>{#if i.ended_at}<Check size={18} />{:else}<TriangleAlert size={18} />{/if}</span
				>
			</div>
			<div class="incident-body">
				<div class="row-between">
					<div>
						<h3>{i.title || i.monitor_name || 'Monitor incident'}</h3>
						<div class="incident-badges">
							<span class="tag severity-{i.severity}">{i.severity || 'major'}</span><span class="tag"
								>{i.phase || (!i.ended_at ? 'investigating' : 'resolved')}</span
							><span class="tag">{i.monitor_id ? 'Automated' : 'Manual'}</span><span class="small-text muted"
								>{#if i.published}<Globe size={12} /> Published{:else}<LockKeyhole size={12} /> Private{/if}</span
							>
						</div>
					</div>
					<span class:green={!!i.ended_at} class:rose={!i.ended_at}>{duration(i.started_at, i.ended_at)}</span
					>
				</div>
				{#if i.public_message}<p>{i.public_message}</p>{:else if i.cause}<p>{i.cause}</p>{/if}
				<div class="incident-times">
					<span>Started {time(i.started_at)}</span>{#if i.ended_at}<span>Resolved {time(i.ended_at)}</span
						>{/if}
				</div>
				{#if i.note}<div class="private-note">
						<strong>Private annotation</strong>
						<p>{i.note}</p>
					</div>{/if}
				<div class="incident-actions">
					<button class="button small" onclick={() => void timeline(i.id)}
						><MessageSquare size={12} /> Show timeline</button
					>{#if !readonly}<button class="button small" onclick={() => edit(i)}
							><Pencil size={12} /> Edit incident</button
						><button class="button small" onclick={() => startUpdate(i)}
							><Plus size={12} /> Post update</button
						>{/if}
				</div>
				{#if updates[i.id]}<div class="incident-update-list">
						{#each updates[i.id] as update}<div>
								<span class="tag">{update.phase}</span><small>{time(update.created_at)}</small>
								<p>{update.message}</p>
							</div>{:else}<p class="muted small-text">
								No editorial updates yet. Automated failure evidence remains in the monitor’s check history.
							</p>{/each}
					</div>{/if}
			</div>
		</article>{:else}<div class="empty-state">
			<div class="empty-icon"><Check size={29} /></div>
			<h3>A clear record when it matters.</h3>
			<p>
				Automated incidents appear after failure thresholds.<br />Create manual incidents for public
				communications and service events.
			</p>
			{#if !readonly}<button class="button primary" onclick={() => edit()}>Create an incident</button>{/if}
		</div>{/each}
</section>
<Modal
	bind:open
	title={editing ? 'Edit incident' : 'Create incident'}
	description="Publish only public-safe details. Private notes and raw monitor errors are never included in public incident updates."
	><form class="form-stack" onsubmit={save}>
		<label
			>Incident title<input
				bind:value={title}
				maxlength="180"
				required
				placeholder="Intermittent API delays"
			/></label
		>
		<div class="form-row">
			<label
				>Severity<select bind:value={severity}
					><option value="minor">Minor</option><option value="major">Major</option><option value="critical"
						>Critical</option
					></select
				></label
			><label
				>Phase<select bind:value={phase}
					><option value="investigating">Investigating</option><option value="identified">Identified</option
					><option value="monitoring">Monitoring</option><option value="resolved">Resolved</option></select
				></label
			>
		</div>
		{#if editing?.monitor_id}<div class="notice">
				This incident is linked to measured checks. It can only be resolved after its monitor recovers. Use a
				resource override to change the public presentation without rewriting evidence.
			</div>{/if}<label
			>Public message<textarea
				bind:value={message}
				maxlength="8000"
				required={published}
				rows="5"
				placeholder="We are investigating intermittent delays affecting some requests."
			></textarea></label
		><label
			>Private annotation<textarea
				bind:value={note}
				maxlength="4000"
				placeholder="Internal context; never published."
			></textarea></label
		><label class="checkbox"
			><input type="checkbox" bind:checked={published} /> Publish incident and its editorial timeline</label
		>
		<fieldset>
			<legend>Status pages</legend>
			<div class="selection-list">
				{#each pages as p}<label class="checkbox"
						><input type="checkbox" value={p.id} bind:group={selectedPages} />{p.title}</label
					>{:else}<p class="muted small-text">Create a status page first to publish an incident.</p>{/each}
			</div>
		</fieldset>
		<fieldset>
			<legend>Affected resources (optional)</legend>
			<div class="selection-list">
				{#each resources.filter((r) => r.public) as r}<label class="checkbox"
						><input type="checkbox" value={r.id} bind:group={selectedResources} />{r.name}</label
					>{/each}
			</div>
			<p class="muted small-text">
				With resources selected, the incident appears only on selected pages containing one of those public
				resources.
			</p>
		</fieldset>
		{#if error}<div class="error" role="alert">{error}</div>{/if}<button
			class="button primary"
			disabled={busy || (published && selectedPages.length === 0)}
			>{busy ? 'Saving…' : 'Save incident'}</button
		>
	</form></Modal
>
<Modal
	bind:open={updateOpen}
	title="Post incident update"
	description={editing?.published
		? 'This message will be visible on the incident’s selected public status pages.'
		: 'This update remains private until the incident is published.'}
	><form class="form-stack" onsubmit={postUpdate}>
		<label
			>Phase<select bind:value={phase}
				><option value="investigating">Investigating</option><option value="identified">Identified</option
				><option value="monitoring">Monitoring</option><option value="resolved">Resolved</option></select
			></label
		><label>Update message<textarea bind:value={message} required maxlength="8000" rows="5"></textarea></label
		>{#if error}<div class="error" role="alert">{error}</div>{/if}<button
			class="button primary"
			disabled={busy}>{busy ? 'Posting…' : 'Post update'}</button
		>
	</form></Modal
>
