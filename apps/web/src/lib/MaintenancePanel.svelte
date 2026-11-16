<script lang="ts">
	import { onMount } from 'svelte';
	import { Plus, Pencil, Trash2, Wrench, Globe } from '@lucide/svelte';
	import { api, time, type Maintenance, type Monitor, type Page, type StatusResource } from './api';
	import Modal from './Modal.svelte';
	let { readonly = false, onmessage }: { readonly?: boolean; onmessage: (s: string) => void } = $props();
	let windows = $state<Maintenance[]>([]);
	let monitors = $state<Monitor[]>([]);
	let resources = $state<StatusResource[]>([]);
	let pages = $state<Page[]>([]);
	let open = $state(false);
	let editing = $state<string | null>(null);
	let reason = $state('');
	let title = $state('');
	let publicMessage = $state('');
	let starts = $state('');
	let ends = $state('');
	let selectedMonitors = $state<string[]>([]);
	let selectedResources = $state<string[]>([]);
	let selectedPages = $state<string[]>([]);
	let error = $state('');
	let busy = $state(false);
	async function load() {
		try {
			[windows, monitors, resources, pages] = await Promise.all([
				api<Maintenance[]>('/maintenance'),
				api<Monitor[]>('/monitors'),
				api<StatusResource[]>('/resources'),
				api<Page[]>('/status-pages')
			]);
		} catch (e) {
			error = (e as Error).message;
		}
	}
	onMount(() => {
		void load();
	});
	function localDate(s: string) {
		const d = new Date(s);
		return new Date(d.getTime() - d.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
	}
	function edit(w?: Maintenance) {
		editing = w?.id || null;
		reason = w?.reason || '';
		title = w?.title || '';
		publicMessage = w?.public_message || '';
		starts = w ? localDate(w.starts_at) : '';
		ends = w ? localDate(w.ends_at) : '';
		selectedMonitors = w?.monitor_ids || [];
		selectedResources = w?.resource_ids || [];
		selectedPages = w?.status_page_ids || [];
		error = '';
		open = true;
	}
	async function save(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = '';
		try {
			await api('/maintenance' + (editing ? '/' + editing : ''), editing ? 'PUT' : 'POST', {
				reason,
				title,
				public_message: publicMessage,
				starts_at: new Date(starts).toISOString(),
				ends_at: new Date(ends).toISOString(),
				monitor_ids: selectedMonitors,
				resource_ids: selectedResources,
				status_page_ids: selectedPages
			});
			open = false;
			await load();
			onmessage('Maintenance window saved');
		} catch (e) {
			error = (e as Error).message;
		} finally {
			busy = false;
		}
	}
	async function remove(w: Maintenance) {
		if (!confirm('Cancel this maintenance window? Incident suppression ends on the next check.')) return;
		try {
			await api('/maintenance/' + w.id, 'DELETE');
			await load();
			onmessage('Maintenance cancelled');
		} catch (e) {
			onmessage((e as Error).message);
		}
	}
</script>

<div class="page-heading">
	<div>
		<div class="eyebrow">MAKE ROOM FOR BETTER</div>
		<h1>Planned, not unexpected.</h1>
		<p class="muted">Schedule work, suppress incident noise, and publish a safe maintenance notice.</p>
	</div>
	{#if !readonly}<button class="button primary" onclick={() => edit()}
			><Plus size={15} /> Schedule maintenance</button
		>{/if}
</div>
{#if error && !open}<div class="error" role="alert">{error}</div>{/if}
<section class="panel">
	<div class="table-scroll">
		<table>
			<thead><tr><th>Window</th><th>Starts</th><th>Ends</th><th>Coverage</th><th></th></tr></thead><tbody
				>{#each windows as w}<tr
						><td
							><strong>{w.title || w.reason}</strong><small class="muted"
								>{new Date(w.ends_at).getTime() < Date.now()
									? 'Completed'
									: new Date(w.starts_at).getTime() <= Date.now()
										? 'In progress'
										: 'Scheduled'}{w.status_page_ids?.length ? ' · Public notice' : ''}</small
							></td
						><td>{time(w.starts_at)}</td><td>{time(w.ends_at)}</td><td
							>{w.monitor_ids.length} monitors<small class="muted"
								>{w.resource_ids?.length || 0} resources</small
							></td
						><td
							>{#if !readonly}<div class="button-group">
									<button class="button small" onclick={() => edit(w)}><Pencil size={12} /> Edit</button
									><button
										class="icon-button danger"
										aria-label="Cancel maintenance"
										onclick={() => void remove(w)}><Trash2 size={14} /></button
									>
								</div>{/if}</td
						></tr
					>{:else}<tr
						><td colspan="5"
							><div class="empty-state">
								<div class="empty-icon"><Wrench size={28} /></div>
								<h3>No planned work on the calendar.</h3>
								<p>Schedule a window before your next deployment.</p>
							</div></td
						></tr
					>{/each}</tbody
			>
		</table>
	</div>
</section>
<Modal
	bind:open
	title={editing ? 'Edit maintenance window' : 'Schedule maintenance'}
	description="Checks continue during maintenance. Public notices contain only the title and description you explicitly publish."
	><form class="form-stack" onsubmit={save}>
		<label
			>Internal reason<input
				bind:value={reason}
				required
				maxlength="500"
				placeholder="Database migration — private operational context"
			/></label
		>
		<div class="form-row">
			<label>Starts (local time)<input type="datetime-local" bind:value={starts} required /></label><label
				>Ends (local time)<input type="datetime-local" bind:value={ends} required /></label
			>
		</div>
		<fieldset>
			<legend>Affected resources</legend>
			<div class="selection-list">
				{#each resources as r}<label class="checkbox"
						><input type="checkbox" value={r.id} bind:group={selectedResources} />{r.name}</label
					>{/each}
			</div>
			<p class="muted small-text">
				Linked monitors are captured when you save. Update the window if you later change resource links.
			</p>
		</fieldset>
		<fieldset>
			<legend>Additional monitors</legend>
			<div class="selection-list">
				{#each monitors as m}<label class="checkbox"
						><input type="checkbox" value={m.id} bind:group={selectedMonitors} />{m.name}</label
					>{/each}
			</div>
		</fieldset>
		<details open={selectedPages.length > 0}>
			<summary>Publish maintenance notice</summary>
			<div class="form-stack inset">
				<label
					>Public title<input
						bind:value={title}
						maxlength="180"
						required={selectedPages.length > 0}
						placeholder="Scheduled platform maintenance"
					/></label
				><label
					>Public description<textarea
						bind:value={publicMessage}
						maxlength="8000"
						required={selectedPages.length > 0}
						placeholder="Some services may be briefly unavailable during this window."
					></textarea></label
				>
				<fieldset>
					<legend>Public status pages</legend>{#each pages as p}<label class="checkbox"
							><input type="checkbox" value={p.id} bind:group={selectedPages} /><Globe
								size={12}
							/>{p.title}</label
						>{/each}
				</fieldset>
			</div>
		</details>
		{#if error}<div class="error" role="alert">{error}</div>{/if}<button
			class="button primary"
			disabled={busy}>{busy ? 'Saving…' : 'Save maintenance window'}</button
		>
	</form></Modal
>
