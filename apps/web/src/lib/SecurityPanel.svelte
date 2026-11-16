<script lang="ts">
	import { onMount } from 'svelte';
	import { ShieldCheck, RotateCcw, LogOut, RefreshCw } from '@lucide/svelte';
	import { api, time, type SessionInfo, type AuditEntry } from './api';
	let { owner = false, onmessage }: { owner?: boolean; onmessage: (s: string) => void } = $props();
	let sessions = $state<SessionInfo[]>([]);
	let audit = $state<AuditEntry[]>([]);
	let error = $state('');
	let totpEnabled = $state(false);
	let totpPassword = $state('');
	let totpCode = $state('');
	let totpSecret = $state('');
	let totpUri = $state('');
	let totpBusy = $state(false);
	async function load() {
		try {
			sessions = await api<SessionInfo[]>('/auth/sessions');
			totpEnabled = (await api<{ enabled: boolean }>('/auth/totp')).enabled;
			if (owner) audit = await api<AuditEntry[]>('/audit');
		} catch (e) {
			error = (e as Error).message;
		}
	}
	async function setupTotp() {
		totpBusy = true;
		error = '';
		try {
			const result = await api<{ secret: string; uri: string }>('/auth/totp/setup', 'POST', { password: totpPassword });
			totpSecret = result.secret;
			totpUri = result.uri;
			totpCode = '';
			totpPassword = '';
		} catch (e) { error = (e as Error).message; }
		finally { totpBusy = false; }
	}
	async function enableTotp() {
		totpBusy = true;
		error = '';
		try {
			await api('/auth/totp/enable', 'POST', { code: totpCode });
			totpEnabled = true;
			totpSecret = '';
			totpUri = '';
			totpCode = '';
			onmessage('Authenticator verification enabled.');
		} catch (e) { error = (e as Error).message; }
		finally { totpBusy = false; }
	}
	async function disableTotp() {
		totpBusy = true;
		error = '';
		try {
			await api('/auth/totp/disable', 'POST', { password: totpPassword, code: totpCode });
			totpEnabled = false;
			totpPassword = '';
			totpCode = '';
			onmessage('Authenticator verification disabled.');
		} catch (e) { error = (e as Error).message; }
		finally { totpBusy = false; }
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
<section class="panel settings-panel full-grid">
	<h2><ShieldCheck size={16} /> Authenticator verification</h2>
	<p class="muted small-text">Use a time-based authenticator app. Codes use SHA-256, six digits and a 30-second period. Save your account recovery procedure before enabling.</p>
	{#if totpEnabled}
		<form class="form-stack" onsubmit={(e) => { e.preventDefault(); void disableTotp(); }}>
			<p>Enabled for your account.</p>
			<label>Current password<input type="password" autocomplete="current-password" bind:value={totpPassword} required /></label>
			<label>Current authenticator code<input inputmode="numeric" autocomplete="one-time-code" pattern="[0-9]{6}" maxlength="6" bind:value={totpCode} required /></label>
			<button class="button small" disabled={totpBusy}>Disable verification</button>
		</form>
	{:else if totpSecret}
		<div class="form-stack">
			<p>Add this secret to your authenticator app, then enter its current code to confirm:</p>
			<code>{totpSecret}</code>
			<details><summary>Provisioning URI</summary><code>{totpUri}</code></details>
			<form class="form-stack" onsubmit={(e) => { e.preventDefault(); void enableTotp(); }}>
				<label>Authenticator code<input inputmode="numeric" autocomplete="one-time-code" pattern="[0-9]{6}" maxlength="6" bind:value={totpCode} required /></label>
				<button class="button small" disabled={totpBusy}>Enable verification</button>
			</form>
		</div>
	{:else}
		<form class="form-stack" onsubmit={(e) => { e.preventDefault(); void setupTotp(); }}>
			<label>Confirm current password<input type="password" autocomplete="current-password" bind:value={totpPassword} required /></label>
			<button class="button small" disabled={totpBusy}>Set up authenticator</button>
		</form>
	{/if}
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
