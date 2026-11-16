<script lang="ts">
	import { onMount } from 'svelte';
	import { ShieldCheck, RotateCcw, LogOut, RefreshCw } from '@lucide/svelte';
	import { api, time, type SessionInfo, type AuditEntry } from './api';
	let { owner = false, onmessage }: { owner?: boolean; onmessage: (s: string) => void } = $props();
	let sessions = $state<SessionInfo[]>([]);
	let audit = $state<AuditEntry[]>([]);
	let error = $state('');
	async function load() {
		try {
			sessions = await api<SessionInfo[]>('/auth/sessions');
			if (owner) audit = await api<AuditEntry[]>('/audit');
		} catch (e) {
			error = (e as Error).message;
		}
	}
	onMount(() => {
		void load();
	});
	async function revoke(s: SessionInfo) {
		if (!confirm(s.current ? 'Sign out this session?' : 'Revoke this session immediately?')) return;
		try {
			await api('/auth/sessions/' + s.id, 'DELETE');
			if (s.current) {
				location.href = '/';
				return;
			}
			await load();
			onmessage('Session revoked');
		} catch (e) {
			onmessage((e as Error).message);
		}
	}
	async function renew() {
		try {
			await api('/auth/refresh', 'POST');
			await load();
			onmessage('Session rotated. The previous cookie is revoked.');
		} catch (e) {
			onmessage((e as Error).message);
		}
	}
</script>

<section class="panel settings-panel full-grid">
	<div class="row-between">
		<h2><ShieldCheck size={16} /> Active sessions</h2>
		<div class="button-group">
			<button class="button small" onclick={() => void renew()}
				><RotateCcw size={12} /> Renew current session</button
			><button class="icon-button" aria-label="Refresh sessions" onclick={() => void load()}
				><RefreshCw size={14} /></button
			>
		</div>
	</div>
	<p class="muted small-text">
		Opaque session tokens are stored only as hashes. Session IDs below cannot authenticate requests.
	</p>
	{#if error}<div class="error" role="alert">{error}</div>{/if}
	<div class="session-list">
		{#each sessions as s}<div class="session-row">
				<div>
					<strong>{s.current ? 'This session' : 'Other authenticated session'}</strong><small
						>Created {time(s.created_at)} · Expires {time(s.expires_at)}</small
					>
				</div>
				<button class="button small" onclick={() => void revoke(s)}
					><LogOut size={12} />{s.current ? 'Sign out' : 'Revoke'}</button
				>
			</div>{/each}
	</div>
</section>
{#if owner}<section class="panel settings-panel full-grid">
		<h2>Administrative audit log</h2>
		<p class="muted small-text">
			Status overrides, editorial incidents, maintenance, channel changes and page changes are attributable to
			an authenticated actor.
		</p>
		<div class="audit-list">
			{#each audit as entry}<div class="audit-row">
					<strong>{entry.action}</strong><small
						>{entry.actor || 'Deleted member'} · {time(entry.created_at)} · {entry.target_id}</small
					>
				</div>{:else}<div class="calm-state">No administrative events recorded yet.</div>{/each}
		</div>
	</section>{/if}
