<script lang="ts">
	import { customSelect } from './customSelect';
	import { onMount } from 'svelte';
	import { Plus, Bell, Pencil, Trash2, RefreshCw, Send, RotateCcw, Eye, Radio } from '@lucide/svelte';
	import {
		api,
		time,
		defaultEmbed,
		type Channel,
		type Delivery,
		type Monitor,
		type EmbedTemplate,
		type DiscordPayload
	} from './api';
	import Modal from './Modal.svelte';
	import EmbedPreview from './EmbedPreview.svelte';
	let { readonly = false, onmessage }: { readonly?: boolean; onmessage: (s: string) => void } = $props();
	let channels = $state<Channel[]>([]);
	let deliveries = $state<Delivery[]>([]);
	let monitors = $state<Monitor[]>([]);
	let open = $state(false);
	let editing = $state<string | null>(null);
	let name = $state('');
	let provider = $state('discord');
	let secret = $state('');
	let enabled = $state(true);
	let template = $state<EmbedTemplate>({ ...defaultEmbed, fields: defaultEmbed.fields.map((field) => ({ ...field })) });
	let events = $state(['DOWN', 'RECOVERED']);
	let routes = $state<string[]>([]);
	let allMonitors = $state(true);
	let monitorSearch = $state('');
	let cooldown = $state(0);
	let quietStart = $state('');
	let quietEnd = $state('');
	let delay = $state(0);
	let reminder = $state(0);
	let previewEvent = $state('DOWN');
	let payload = $state<DiscordPayload | null>(null);
	let error = $state('');
	let busy = $state(false);
	let loading = $state(true);
	async function load() {
		if (readonly) {
			loading = false;
			return;
		}
		try {
			[channels, deliveries, monitors] = await Promise.all([
				api<Channel[]>('/channels'),
				api<Delivery[]>('/deliveries'),
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
		const id = setInterval(() => void load(), 15000);
		return () => clearInterval(id);
	});
	let filteredMonitors = $derived(monitors.filter((m) => `${m.name} ${m.group_name || ''}`.toLowerCase().includes(monitorSearch.toLowerCase())));
	function routedChannels(id: string) {
		return channels.filter((c) => c.enabled && (!c.monitor_ids?.length || c.monitor_ids.includes(id)));
	}
	function edit(c?: Channel, monitorId?: string) {
		editing = c?.id || null;
		name = c?.name || '';
		provider = c?.provider || 'discord';
		secret = '';
		enabled = c?.enabled ?? true;
		template = {
			...defaultEmbed,
			...c?.template,
			fields: (c?.template?.fields || defaultEmbed.fields).map((field) => ({ ...field }))
		};
		events = c?.events || ['DOWN', 'RECOVERED'];
		routes = c?.monitor_ids ? [...c.monitor_ids] : monitorId ? [monitorId] : [];
		allMonitors = !monitorId && !routes.length;
		monitorSearch = '';
		cooldown = c?.cooldown_seconds || 0;
		quietStart = c?.quiet_start_minute == null ? '' : `${String(Math.floor(c.quiet_start_minute / 60)).padStart(2, '0')}:${String(c.quiet_start_minute % 60).padStart(2, '0')}`;
		quietEnd = c?.quiet_end_minute == null ? '' : `${String(Math.floor(c.quiet_end_minute / 60)).padStart(2, '0')}:${String(c.quiet_end_minute % 60).padStart(2, '0')}`;
		delay = c?.delay_seconds || 0;
		reminder = c?.reminder_seconds || 0;
		payload = null;
		error = '';
		open = true;
		void preview();
	}
	async function preview() {
		error = '';
		try {
			payload = await api<DiscordPayload>('/channels/preview', 'POST', { template, event: previewEvent });
		} catch (e) {
			error = (e as Error).message;
		}
	}
	async function save(e: SubmitEvent) {
		e.preventDefault();
		if (Boolean(quietStart) !== Boolean(quietEnd)) { error = 'Set both quiet-hour times or clear both.'; return; }
		busy = true;
		error = '';
		try {
			await api('/channels' + (editing ? '/' + editing : ''), editing ? 'PUT' : 'POST', {
				name,
				provider,
				secret: secret || null,
				enabled,
				template,
				events,
				monitor_ids: allMonitors ? [] : routes,
				cooldown_seconds: cooldown,
				quiet_start_minute: quietStart ? Number(quietStart.slice(0, 2)) * 60 + Number(quietStart.slice(3)) : null,
				quiet_end_minute: quietEnd ? Number(quietEnd.slice(0, 2)) * 60 + Number(quietEnd.slice(3)) : null,
				delay_seconds: delay,
				reminder_seconds: reminder
			});
			open = false;
			secret = '';
			await load();
			onmessage('Notification channel saved. Credentials remain encrypted.');
		} catch (e) {
			error = (e as Error).message;
		} finally {
			busy = false;
		}
	}
	async function test(c: Channel) {
		try {
			await api('/channels/' + c.id + '/test', 'POST');
			onmessage('Test queued. Delivery history shows the provider result.');
			await load();
		} catch (e) {
			onmessage((e as Error).message);
		}
	}
	async function remove(c: Channel) {
		if (!confirm(`Delete ${c.name}? Pending deliveries will no longer be sent.`)) return;
		try {
			await api('/channels/' + c.id, 'DELETE');
			await load();
			onmessage('Channel deleted');
		} catch (e) {
			onmessage((e as Error).message);
		}
	}
	async function retry(id: string) {
		try {
			await api('/deliveries/' + id + '/retry', 'POST');
			await load();
			onmessage('Delivery requeued');
		} catch (e) {
			onmessage((e as Error).message);
		}
	}
</script>

<div class="page-heading">
	<div>
		<div class="eyebrow">THE RIGHT SIGNAL, DELIVERED</div>
		<h1>Stay in the loop.</h1>
		<p class="muted">Design your Discord embeds, choose event routes, and inspect every delivery.</p>
	</div>
	{#if !readonly}<button class="button primary" onclick={() => edit()}><Plus size={16} /> Add channel</button
		>{/if}
</div>
{#if readonly}<div class="notice">
		Notification management requires an Admin or Owner account.
	</div>{:else}{#if error && !open}<div class="error" role="alert">{error}</div>{/if}{#if loading}<div
			class="panel calm-state"
		>
			Loading destinations…
		</div>{:else}<div class="channel-grid">
			{#each channels as c}<section class="panel channel-card">
					<div class="row-between">
						<span class="channel-icon"><Bell size={24} /></span><span class="pill" class:green={c.enabled}
							>{c.enabled ? 'Enabled' : 'Disabled'}</span
						>
					</div>
					<h2>{c.name}</h2>
					<p class="muted">
						{c.provider === 'discord' ? 'Discord embed' : c.provider === 'email' ? 'Email via SMTP' : 'Generic JSON webhook'} · Encrypted destination
					</p>
					<div class="channel-route-detail">
						<span class="route-caption">Sends for</span>
						<div class="route-chips">{#each c.events || [] as event}<span class="route-chip">{event}</span>{/each}</div>
						<span class="route-caption">Monitors</span>
						<p>{c.monitor_ids?.length ? c.monitor_ids.map((id) => monitors.find((m) => m.id === id)?.name || 'Removed monitor').join(', ') : 'All current and future monitors'}</p>
					</div>
					<div class="row-between">
						<button class="button small" disabled={!c.enabled} onclick={() => void test(c)}
							><Send size={13} /> Send test</button
						>
						<div class="button-group">
							<button class="button small" onclick={() => edit(c)}><Pencil size={13} /> Edit</button><button
								class="icon-button danger"
								aria-label={'Delete ' + c.name}
								onclick={() => void remove(c)}><Trash2 size={15} /></button
							>
						</div>
					</div>
				</section>{:else}<section class="panel empty-state full-grid">
					<div class="empty-icon"><Bell size={28} /></div>
					<h3>Make every alert useful.</h3>
					<p>
						Create a Discord destination with your own embed design,<br />or route events to a generic
						webhook.
					</p>
					<button class="button primary" onclick={() => edit()}>Add your first channel</button>
				</section>{/each}
		</div>{/if}
	<section class="panel coverage-panel">
		<div class="panel-heading"><div><h2>Alerts by monitor</h2><p class="muted small-text">See which destinations receive each monitor’s alerts. Create a channel for a different event mix or destination.</p></div></div>
		{#if monitors.length}<div class="coverage-list">
			{#each monitors as monitor}<div class="coverage-row">
				<div class="coverage-monitor"><Radio size={16} /><div><strong>{monitor.name}</strong><small class="muted">{monitor.group_name || monitor.kind.toUpperCase()}</small></div></div>
				<div class="coverage-destinations">{#each routedChannels(monitor.id) as channel}<button class="coverage-channel" onclick={() => edit(channel)} title={'Edit ' + channel.name}>{channel.name}<span>{channel.events.join(' / ')}</span></button>{:else}<span class="muted small-text">No active alerts</span>{/each}</div>
				<button class="button small" onclick={() => edit(undefined, monitor.id)}><Plus size={13} /> Add alert</button>
			</div>{/each}
		</div>{:else}<p class="muted coverage-empty">Add a monitor to configure its alerts.</p>{/if}
	</section>
	<section class="panel">
		<div class="panel-heading">
			<div>
				<h2>Delivery history</h2>
				<p class="muted small-text">
					Durable retries · at least once delivery · templates snapshotted when queued
				</p>
			</div>
			<button class="icon-button" aria-label="Refresh deliveries" onclick={() => void load()}
				><RefreshCw size={15} /></button
			>
		</div>
		<div class="table-scroll">
			<table>
				<thead
					><tr><th>Destination</th><th>Event</th><th>State</th><th>Attempts</th><th>Time</th><th></th></tr
					></thead
				><tbody
					>{#each deliveries as d}<tr
							><td>{d.channel_name || 'Removed channel'}</td><td><span class="tag">{d.event}</span></td><td
								><span class:green={d.state === 'sent'} class:rose={d.state === 'failed'}>{d.state}</span
								>{#if d.last_error}<small class="muted">{d.last_error}</small>{/if}</td
							><td>{d.attempts} / 8</td><td class="muted">{time(d.created_at)}</td><td
								>{#if d.state === 'failed'}<button class="button small" onclick={() => void retry(d.id)}
										><RotateCcw size={13} /> Retry</button
									>{/if}</td
							></tr
						>{:else}<tr
							><td colspan="6" class="empty-cell"
								>No deliveries yet. Send a real test from a saved destination.</td
							></tr
						>{/each}</tbody
				>
			</table>
		</div>
	</section>{/if}
<Modal
	bind:open
	title={editing ? 'Edit notification channel' : 'Create notification channel'}
	description="Edit presentation separately from the encrypted destination. Never put secrets in embed templates."
	><form class="form-stack" onsubmit={save}>
		<div class="form-row">
			<label
				>Name<input bind:value={name} required maxlength="120" placeholder="Infrastructure alerts" /></label
			><label
				>Provider<select use:customSelect bind:value={provider} disabled={!!editing}
					><option value="discord">Discord</option><option value="webhook">Generic webhook</option><option value="email">Email</option></select
				></label
			>
		</div>
		<label
			>{editing ? 'Replace destination (leave blank to keep)' : provider === 'email' ? 'Recipient email address' : 'Webhook URL'}<input
				type={provider === 'email' ? 'email' : 'password'}
				autocomplete="off"
				bind:value={secret}
				required={!editing}
				placeholder={provider === 'email' ? 'alerts@example.com' : provider === 'discord'
					? 'https://discord.com/api/webhooks/…'
					: 'https://hooks.example.com/…'}
			/></label
		><label class="checkbox"><input type="checkbox" bind:checked={enabled} /> Enabled</label>
		<fieldset>
			<legend>Events to send</legend>
			<div class="check-options">
					{#each ['DOWN', 'RECOVERED', 'DEGRADED', 'SLO_BURN'] as event}<label class="checkbox"
						><input type="checkbox" value={event} bind:group={events} />{event}</label
					>{/each}
			</div>
		</fieldset>
		<fieldset>
			<legend>Which monitors send to this channel?</legend>
			<div class="routing-options"><label class="routing-choice" class:selected={allMonitors}><input type="radio" name="monitor-scope" checked={allMonitors} onchange={() => (allMonitors = true)} /><span><strong>All monitors</strong><small>Includes monitors added later</small></span></label><label class="routing-choice" class:selected={!allMonitors}><input type="radio" name="monitor-scope" checked={!allMonitors} onchange={() => (allMonitors = false)} /><span><strong>Selected monitors</strong><small>Only the monitors you choose below</small></span></label></div>
			{#if !allMonitors}<div class="monitor-picker-heading"><span>{routes.length} selected · 200 maximum</span><div class="button-group"><button type="button" class="text-button" onclick={() => (routes = monitors.slice(0, 200).map((m) => m.id))}>{monitors.length > 200 ? 'Select first 200' : 'Select all'}</button><button type="button" class="text-button" onclick={() => (routes = [])}>Clear</button></div></div><input aria-label="Search monitors" placeholder="Search monitors…" bind:value={monitorSearch} /><div class="selection-list monitor-picker">
					{#each filteredMonitors as m}<label class="checkbox"
							><input type="checkbox" value={m.id} bind:group={routes} disabled={!routes.includes(m.id) && routes.length >= 200} /><span>{m.name}</span><span class="muted">{m.group_name || m.kind.toUpperCase()}</span></label
						>{:else}<p class="muted small-text">No matching monitors.</p>{/each}
				</div>
				<p class="muted small-text">Select at least one monitor to save this route.</p>{/if}<label
				>Down-alert cooldown (seconds)<input type="number" bind:value={cooldown} min="0" max="86400" /></label
			>
			<p class="muted small-text">
				Suppress repeated DOWN alerts for the same monitor during a cooldown. Recoveries still notify.
			</p>
			<div class="form-row"><label>Quiet hours start (UTC)<input type="time" bind:value={quietStart} /></label><label>Quiet hours end (UTC)<input type="time" bind:value={quietEnd} /></label></div>
			<p class="muted small-text">During quiet hours, queued alerts wait until the end time. Test messages bypass quiet hours.</p>
			<label>DOWN escalation delay (seconds)<input type="number" bind:value={delay} min="0" max="86400" /></label>
			<p class="muted small-text">To escalate, add a second channel with a delay. Pending DOWN messages are cancelled when the incident recovers.</p>
			<label>Repeat DOWN reminder (seconds)<input type="number" bind:value={reminder} min="0" max="86400" step="60" /></label>
			<p class="muted small-text">0 disables reminders. Otherwise enter at least 300 seconds. Reminders stop when the incident recovers.</p>
		</fieldset>
		{#if provider === 'discord'}<details open>
				<summary>Discord message designer</summary>
				<div class="form-stack inset">
					<div class="template-variables">
						Variables: {#each ['monitor', 'status', 'reason', 'duration', 'timestamp', 'dashboard_url', 'brand', 'incident_id'] as token}<code
								>{'{{' + token + '}}'}</code
							>{/each}
					</div>
					<label>Message content (outside embed)<textarea bind:value={template.content} maxlength="2000" rows="3" placeholder={'Optional message shown above the embed. Use {{monitor}} or {{status}}.'}></textarea></label>
					<fieldset>
						<legend>Mentions from message content</legend>
						<div class="check-options"><label class="checkbox"><input type="checkbox" bind:checked={template.mention_users} /> Ping mentioned users</label><label class="checkbox"><input type="checkbox" bind:checked={template.mention_roles} /> Ping mentioned roles</label><label class="checkbox"><input type="checkbox" bind:checked={template.mention_everyone} /> Ping @everyone / @here</label></div>
						<p class="muted small-text">Use Discord IDs in the content: &lt;@USER_ID&gt; for a user or &lt;@&amp;ROLE_ID&gt; for a role. Only mentions you write in the content can ping; variables cannot add pings.</p>
					</fieldset>
					<label>Embed title<input bind:value={template.title} maxlength="256" required /></label><label
						>Description<textarea bind:value={template.description} maxlength="4096" rows="4"
						></textarea></label
					>
					<div class="form-row">
						<label>Bot display name<input bind:value={template.username} maxlength="80" /></label><label
							>Footer<input bind:value={template.footer} maxlength="2048" /></label
						>
					</div>
					<div class="form-row">
						<label
							>Avatar HTTPS URL<input
								type="url"
								bind:value={template.avatar_url}
								placeholder="https://…"
							/></label
						><label
							>Thumbnail HTTPS URL<input
								type="url"
								bind:value={template.thumbnail_url}
								placeholder="https://…"
							/></label
						>
					</div>
					<div class="form-row three">
						<label>Down color<input type="color" bind:value={template.down_color} /></label><label
							>Recovered color<input type="color" bind:value={template.recovered_color} /></label
						><label>Degraded color<input type="color" bind:value={template.degraded_color} /></label>
					</div>
					<label class="checkbox"
						><input type="checkbox" bind:checked={template.show_timestamp} /> Include timestamp</label
					>
					<div class="section-heading">
						<h3>Embed fields</h3>
						<button
							class="button small"
							type="button"
							disabled={template.fields.length >= 15}
							onclick={() =>
								(template.fields = [
									...template.fields,
									{ name: 'Field', value: '{{reason}}', inline: false }
								])}><Plus size={12} /> Add field</button
						>
					</div>
					{#each template.fields as field, i}<div class="editor-field">
							<div class="form-row">
								<label>Field name<input bind:value={field.name} maxlength="256" required /></label><label
									>Field value<input bind:value={field.value} maxlength="1024" required /></label
								>
							</div>
							<div class="row-between">
								<label class="checkbox"><input type="checkbox" bind:checked={field.inline} /> Inline</label
								><button
									type="button"
									class="icon-button danger"
									aria-label={'Remove field ' + (i + 1)}
									onclick={() => (template.fields = template.fields.filter((_, j) => j !== i))}
									><Trash2 size={14} /></button
								>
							</div>
						</div>{/each}
					<div class="row-between">
						<label
							>Preview event<select use:customSelect bind:value={previewEvent}
								><option>DOWN</option><option>RECOVERED</option><option>DEGRADED</option><option>SLO_BURN</option><option>TEST</option
								></select
							></label
						><button type="button" class="button" onclick={() => void preview()}
						><Eye size={14} /> Preview message</button
						>
					</div>
					<EmbedPreview {payload} />
					<p class="muted small-text">
						Server-validated preview; it does not send a ping. Selected mentions can ping when delivered. Images are fetched by Discord only
						after delivery; avoid URLs containing secrets.
					</p>
				</div>
			</details>{/if}
		{#if error}<div class="error" role="alert">{error}</div>{/if}
		<div class="form-actions">
			<button
				class="button primary"
				disabled={busy || (!allMonitors && routes.length === 0) || events.length === 0}
				>{busy ? 'Saving…' : 'Save channel'}</button
			>
		</div>
	</form></Modal
>
