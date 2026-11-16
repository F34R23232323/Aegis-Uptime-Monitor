<script lang="ts">
	import { customSelect } from './customSelect';
	import { untrack } from 'svelte';
	import { Plus, Trash2, LockKeyhole } from '@lucide/svelte';
	import { api, type Monitor, type Server } from './api';
	let {
		monitor,
		servers = [],
		monitors = [],
		onsave,
		onmessage
	}: {
		monitor?: Partial<Monitor>;
		servers?: Server[];
		monitors?: Monitor[];
		onsave: () => void;
		onmessage: (s: string) => void;
	} = $props();
	const initial = untrack(() => monitor);
	let name = $state(initial?.name || '');
	let kind = $state(initial?.kind || 'http');
	let target = $state(initial?.target || '');
	let interval = $state(initial?.interval_seconds || 30);
	let timeout = $state(initial?.timeout_seconds || 10);
	let failures = $state(initial?.failure_threshold || 2);
	let recovery = $state(initial?.recovery_threshold || 2);
	let group = $state(initial?.group_name || '');
	let tags = $state(initial?.tags?.join(', ') || '');
	let description = $state(initial?.description || '');
	let publicMonitor = $state(initial?.public || false);
	let server = $state(initial?.server_id || '');
	let dependsOn = $state(initial?.depends_on || '');
	let method = $state(String(initial?.config?.method || 'GET'));
	let status = $state(Number(initial?.config?.status || 200));
	let keyword = $state(String(initial?.config?.keyword || ''));
	let expected = $state(String(initial?.config?.expected || ''));
	let recordType = $state(String(initial?.config?.record_type || 'IP'));
	let jsonPointer = $state(String(initial?.config?.json_pointer || '/status'));
	let jsonExpected = $state(JSON.stringify(initial?.config?.json_expected ?? 'ok'));
	let jsonComponentId = $state(String(initial?.config?.json_component_id || ''));
	let jsonGoogleProductId = $state(String(initial?.config?.json_google_product_id || ''));
	let warning = $state(Number(initial?.config?.warning_days ?? 14));
	let heartbeatGrace = $state(
		Number(initial?.config?.heartbeat_grace_seconds ?? initial?.interval_seconds ?? 30)
	);
	let checkRetries = $state(Number(initial?.config?.check_retries ?? 0));
	let followRedirects = $state(Boolean(initial?.config?.follow_redirects ?? false));
	let retentionDays = $state(Number(initial?.config?.retention_days ?? 90));
	let sloEnabled = $state(initial?.config?.slo_target_percent != null);
	let sloTarget = $state(Number(initial?.config?.slo_target_percent ?? 99.9));
	let sloWindow = $state(Number(initial?.config?.slo_window_days ?? 30));
	let sloAlert = $state(Number(initial?.config?.slo_alert_percent ?? 75));
	let cpu = $state(Number(initial?.config?.cpu_threshold || 95));
	let memory = $state(Number(initial?.config?.memory_threshold || 95));
	let disk = $state(Number(initial?.config?.disk_threshold || 90));
	let mount = $state(String(initial?.config?.disk_mount || ''));
	let loadThreshold = $state(Number(initial?.config?.load_threshold || 0));
	let credentialAction = $state('keep');
	let authMode = $state('none');
	let bearer = $state('');
	let basicUsername = $state('');
	let basicPassword = $state('');
	let headers = $state<{ name: string; value: string }[]>([]);
	let busy = $state(false);
	let error = $state('');
	let test = $state('');
	let push = $state('');
	let agent = $derived(['docker', 'pm2', 'host', 'cpu', 'memory', 'disk'].includes(kind));
	let selectedServer = $derived(servers.find((s) => s.id === server));
	let discoveredServices = $derived(
		kind === 'pm2'
			? selectedServer?.inventory.pm2 || []
			: kind === 'docker'
				? selectedServer?.inventory.docker || []
				: []
	);
	let http = $derived(['http', 'json'].includes(kind));
	const types = [
		['http', 'HTTP / HTTPS'],
		['json', 'JSON API assertion'],
		['tcp', 'TCP port'],
		['dns', 'DNS record'],
		['tls', 'TLS certificate'],
		['redis', 'Redis PING'],
		['smtp', 'SMTP greeting'],
		['postgres', 'PostgreSQL handshake'],
		['push', 'Push heartbeat'],
		['docker', 'Docker container'],
		['pm2', 'PM2 application'],
		['host', 'Host health'],
		['cpu', 'CPU usage'],
		['memory', 'Memory usage'],
		['disk', 'Disk space']
	];
	function payload() {
		let config: Record<string, unknown> = {};
		if (http) {
			config = {
				method: kind === 'json' ? 'GET' : method,
				status,
				keyword,
				follow_redirects: followRedirects
			};
			if (kind === 'json') {
				if (jsonComponentId) config.json_component_id = jsonComponentId;
				else if (jsonGoogleProductId) config.json_google_product_id = jsonGoogleProductId;
				else {
					config.json_pointer = jsonPointer;
					config.json_expected = JSON.parse(jsonExpected);
				}
			}
		} else if (kind === 'dns') config = { record_type: recordType, expected };
		else if (kind === 'tls') config = { warning_days: warning };
		else if (kind === 'push') config = { heartbeat_grace_seconds: heartbeatGrace };
		else if (['host', 'cpu', 'memory', 'disk'].includes(kind)) {
			config = { cpu_threshold: cpu, memory_threshold: memory };
			if (kind === 'disk') {
				config.disk_threshold = disk;
				config.disk_mount = mount;
			}
			if (kind === 'host' && loadThreshold > 0) config.load_threshold = loadThreshold;
		}
		if (['http', 'json', 'tcp', 'dns', 'tls', 'redis', 'smtp', 'postgres'].includes(kind))
			config.check_retries = checkRetries;
		config.retention_days = retentionDays;
		if (sloEnabled) {
			config.slo_target_percent = sloTarget;
			config.slo_window_days = sloWindow;
			config.slo_alert_percent = sloAlert;
		}
		let credentials: Record<string, unknown> | undefined;
		if (http && credentialAction === 'clear') credentials = { clear: true };
		else if (http && credentialAction === 'replace')
			credentials = {
				bearer: authMode === 'bearer' ? bearer : '',
				basic_username: authMode === 'basic' ? basicUsername : '',
				basic_password: authMode === 'basic' ? basicPassword : '',
				headers
			};
		return {
			name,
			kind,
			target,
			interval_seconds: interval,
			timeout_seconds: timeout,
			failure_threshold: failures,
			recovery_threshold: recovery,
			group_name: group,
			description,
			tags: tags
				.split(',')
				.map((s) => s.trim())
				.filter(Boolean),
			public: publicMonitor,
			server_id: agent ? server : null,
			depends_on: dependsOn || null,
			config,
			credentials,
			monitor_id: monitor?.id
		};
	}
	async function save(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = '';
		try {
			const r = await api<{ push_token?: string }>(
				'/monitors' + (monitor?.id ? '/' + monitor.id : ''),
				monitor?.id ? 'PUT' : 'POST',
				payload()
			);
			if (r.push_token) {
				push = `${location.origin}/api/v1/push/${r.push_token}`;
				onmessage('Monitor created. Copy the push URL before closing.');
			} else {
				onsave();
				onmessage(monitor?.id ? 'Monitor updated' : 'Monitor created. The scheduler will check it shortly.');
			}
		} catch (e) {
			error = (e as Error).message;
		} finally {
			busy = false;
		}
	}
	async function testConnection() {
		busy = true;
		error = '';
		test = '';
		try {
			const r = await api<{
				ok: boolean;
				latency_ms: number;
				error: string;
				observed: Record<string, unknown>;
			}>('/monitors/test', 'POST', payload());
			test = r.ok
				? `Connection successful · ${r.latency_ms.toFixed(1)} ms${kind === 'dns' ? ' · ' + JSON.stringify(r.observed.records) : ''}`
				: r.error;
		} catch (e) {
			error = (e as Error).message;
		} finally {
			busy = false;
		}
	}
</script>

{#if push}<div class="notice">
		Treat this URL as a password. It is shown once. Send an HTTP POST every {interval} seconds; grace is {heartbeatGrace}
		seconds.
	</div>
	<code class="secret">{push}</code><button class="button primary" onclick={onsave}
		>I have saved the URL</button
	>{:else}<form onsubmit={save} class="form-stack">
		<div class="form-row">
			<label>Name<input bind:value={name} required maxlength="120" placeholder="Production API" /></label
			><label
				>Monitor type{#if monitor?.id}<span class="readonly-value"
						>{types.find(([value]) => value === kind)?.[1] || kind}</span
					>{:else}<select use:customSelect bind:value={kind}
						>{#each types as [value, label]}<option {value}>{label}</option>{/each}</select
					>{/if}</label
			>
		</div>
		{#if agent}<label
				>Enrolled server<select use:customSelect bind:value={server} required
					><option value="">Select a server</option>{#each servers as s}<option value={s.id}>{s.name}</option
						>{/each}</select
				></label
			>{/if}
		{#if kind === 'pm2' || kind === 'docker'}
			<label>
				{kind === 'pm2' ? 'PM2 application' : 'Docker container'}
				<select use:customSelect bind:value={target} required disabled={!server || (!discoveredServices.length && !target)}>
					<option value=""
						>{!server
							? 'Select a server first'
							: !discoveredServices.length
								? 'No services discovered yet'
								: 'Select an application'}</option
					>
					{#each discoveredServices as service}
						<option value={service.id}>{service.name || service.id} · {service.id}</option>
					{/each}
					{#if target && !discoveredServices.some((service) => service.id === target)}
						<option value={target}>Saved ID: {target} (not currently discovered)</option>
					{/if}
				</select>
			</label>
			{#if server && !discoveredServices.length}
				<p class="muted small-text">
					The selected server has not reported any {kind === 'pm2'
						? 'PM2 applications'
						: 'Docker containers'}. Check its agent heartbeat and discovery settings.
				</p>
			{/if}
		{:else if !['host', 'cpu', 'memory', 'disk', 'push'].includes(kind)}
			<label>
				{agent ? 'Discovered service ID' : http ? 'URL' : kind === 'dns' ? 'Hostname' : 'Hostname and port'}
				<input
					bind:value={target}
					required
					placeholder={http
						? 'https://api.example.com/health'
						: kind === 'dns'
							? 'example.com'
							: kind === 'redis'
								? 'example.com:6379'
								: kind === 'smtp'
									? 'example.com:25'
									: kind === 'postgres'
										? 'example.com:5432'
										: 'example.com:443'}
				/>
			</label>
		{/if}
		{#if http}<div class="form-row">
				{#if kind === 'http'}<label
						>Method<select use:customSelect bind:value={method}><option>GET</option><option>HEAD</option></select></label
					>{/if}<label
					>Expected status<input type="number" bind:value={status} min="100" max="599" required /></label
				>
			</div>
			<label class="checkbox"
				><input type="checkbox" bind:checked={followRedirects} /> Follow up to three HTTP redirects</label
			>
			{#if kind === 'json' && jsonComponentId}<div class="notice">
					Watching provider component ID <code>{jsonComponentId}</code>. The check resolves the component by
					ID on every poll and alerts when it is no longer operational.
				</div>{/if}
			{#if kind === 'json' && jsonGoogleProductId}<div class="notice">
					Watching Google Cloud product ID <code>{jsonGoogleProductId}</code> in the public incident feed. Project-specific
					issues may not appear there.
				</div>{/if}
			{#if kind === 'json' && !jsonComponentId && !jsonGoogleProductId}<label
					>JSON Pointer<input bind:value={jsonPointer} maxlength="512" placeholder="/status" /></label
				><label
					>Expected JSON value<textarea
						bind:value={jsonExpected}
						required
						maxlength="16384"
						rows="2"
						placeholder={'"ok"'}
					></textarea></label
				>
				<p class="muted small-text">
					Use valid JSON: a quoted string, number, boolean, null, array or object. Matching is exact at the
					pointer; response reads are bounded to 256 KiB.
				</p>{/if}<label
				>Body must contain (optional)<input
					bind:value={keyword}
					maxlength="4096"
					placeholder="Bounded keyword assertion"
				/></label
			>
			<details>
				<summary>Encrypted authentication & headers{initial?.has_credentials ? ' · configured' : ''}</summary>
				<div class="form-stack inset">
					<div class="notice credentials-notice">
						<LockKeyhole size={15} /> Credentials are encrypted separately and never returned by GET APIs. Do not
						put secrets in URLs, descriptions or embed templates.
					</div>
					<label
						>Credentials<select use:customSelect bind:value={credentialAction}
							><option value="keep"
								>{initial?.has_credentials ? 'Keep saved credentials' : 'No credentials'}</option
							><option value="replace">Set / replace credentials</option>{#if initial?.has_credentials}<option
									value="clear">Remove saved credentials</option
								>{/if}</select
						></label
					>{#if credentialAction === 'replace'}<label
							>HTTP authentication<select use:customSelect bind:value={authMode}
								><option value="none">Headers only / no authentication</option><option value="bearer"
									>Bearer token</option
								><option value="basic">Basic authentication</option></select
							></label
						>{#if authMode === 'bearer'}<label
								>Bearer token<input
									type="password"
									autocomplete="off"
									bind:value={bearer}
									maxlength="8192"
									required
								/></label
							>{:else if authMode === 'basic'}<div class="form-row">
								<label
									>Basic username<input
										bind:value={basicUsername}
										autocomplete="off"
										maxlength="256"
										required
									/></label
								><label
									>Basic password<input
										type="password"
										bind:value={basicPassword}
										autocomplete="off"
										maxlength="2048"
									/></label
								>
							</div>{/if}
						<div class="section-heading">
							<h3>Custom secret headers</h3>
							<button
								type="button"
								class="button small"
								disabled={headers.length >= 20}
								onclick={() => (headers = [...headers, { name: '', value: '' }])}
								><Plus size={12} /> Add header</button
							>
						</div>
						{#each headers as h, i}<div class="credential-header">
								<div class="form-row">
									<label>Header name<input bind:value={h.name} required placeholder="X-API-Key" /></label
									><label
										>Header value<input
											type="password"
											autocomplete="off"
											bind:value={h.value}
											maxlength="4096"
											required
										/></label
									>
								</div>
								<button
									type="button"
									class="icon-button danger"
									aria-label="Remove secret header"
									onclick={() => (headers = headers.filter((_, j) => j !== i))}><Trash2 size={13} /></button
								>
							</div>{/each}
						<p class="muted small-text">
							Replacing credentials replaces the entire saved credential set. Host, Cookie, connection and
							framing headers are forbidden.
						</p>{/if}
				</div>
			</details>
		{:else if kind === 'dns'}<div class="form-row">
				<label
					>DNS record type<select use:customSelect bind:value={recordType}
						>{#each ['IP', 'A', 'AAAA', 'CNAME', 'MX', 'TXT', 'NS', 'SRV'] as value}<option {value}
								>{value === 'IP' ? 'A + AAAA' : value}</option
							>{/each}</select
					></label
				><label>Expected value (optional)<input bind:value={expected} maxlength="4096" /></label>
			</div>
			<p class="muted small-text">
				Uses the server’s configured resolver. Copy the exact record value from a connection test; comparison
				ignores case and a trailing dot.
			</p>
		{:else if kind === 'tls'}<label
				>Fail below remaining certificate days<input
					type="number"
					bind:value={warning}
					min="0"
					max="365"
					required
				/></label
			>
		{:else if ['host', 'cpu', 'memory', 'disk'].includes(kind)}<div class="form-row">
				{#if kind === 'host' || kind === 'cpu'}<label
						>CPU threshold %<input type="number" bind:value={cpu} min="1" max="100" required /></label
					>{/if}{#if kind === 'host' || kind === 'memory'}<label
						>RAM threshold %<input type="number" bind:value={memory} min="1" max="100" required /></label
					>{/if}
			</div>
			{#if kind === 'host'}<label
					>Load threshold (0 disables)<input
						type="number"
						bind:value={loadThreshold}
						min="0"
						max="10000"
						step="0.1"
					/></label
				>{/if}{#if kind === 'disk'}<label
					>Disk usage threshold %<input type="number" bind:value={disk} min="1" max="100" required /></label
				><label
					>Disk mount (blank checks all reported disks)<input
						bind:value={mount}
						maxlength="1024"
						placeholder="/"
					/></label
				>{/if}
			<p class="muted small-text">
				Requires a fresh authenticated agent heartbeat. Missing metrics are failures, never healthy defaults.
			</p>
		{:else if kind === 'redis'}<div class="notice">
				Sends only the non-mutating Redis PING command and requires PONG. Authenticated Redis deployments can
				use a TCP monitor for reachability; protocol authentication is not implemented.
			</div>{:else if kind === 'smtp'}<div class="notice">
				Verifies an SMTP 220 greeting. No email is sent, no authentication is attempted, and STARTTLS is not
				negotiated.
			</div>{:else if kind === 'postgres'}<div class="notice">
				Verifies the PostgreSQL SSLRequest protocol response. This confirms protocol availability, not
				database authentication or query health.
			</div>{/if}
		<div class="form-row">
			<label
				>Check interval (seconds)<input
					type="number"
					bind:value={interval}
					min="20"
					max="86400"
					required
				/></label
			><label>Timeout (seconds)<input type="number" bind:value={timeout} min="1" max="30" required /></label>
		</div>
		{#if kind === 'push'}<label
				>Heartbeat grace (seconds)<input
					type="number"
					bind:value={heartbeatGrace}
					min="0"
					max="86400"
					required
				/></label
			>{/if}
		<details>
			<summary>Organization & advanced settings</summary>
			<div class="form-stack inset">
				<label>Alert dependency<select use:customSelect bind:value={dependsOn}><option value="">None</option>{#each monitors.filter((m) => m.id !== monitor?.id) as m}<option value={m.id}>{m.name}</option>{/each}</select></label>
				<p class="muted small-text">If this monitor goes down while its dependency is down, its DOWN notification waits. Its incident and checks remain visible.</p>
				<label
					>Raw check retention (days)<input
						type="number"
						bind:value={retentionDays}
						min="7"
						max="365"
						required
					/></label
				>
				<label class="checkbox"><input type="checkbox" bind:checked={sloEnabled} /> Track an availability SLO and alert on error-budget use</label>
				{#if sloEnabled}<div class="form-row three">
					<label>Target availability %<input type="number" bind:value={sloTarget} min="90" max="99.999" step="0.001" required /></label>
					<label>Window<select use:customSelect bind:value={sloWindow}><option value={7}>7 days</option><option value={30}>30 days</option><option value={90}>90 days</option></select></label>
					<label>Alert at budget used %<input type="number" bind:value={sloAlert} min="1" max="100" required /></label>
				</div><p class="muted small-text">Budget use counts only confirmed UP and DOWN time. Choose the SLO_BURN event on a channel to receive at most one alert per monitor each day after an hour of measured coverage.</p>{/if}
				{#if ['http', 'json', 'tcp', 'dns', 'tls', 'redis', 'smtp', 'postgres'].includes(kind)}
					<label
						>Retries within each observation<input
							type="number"
							bind:value={checkRetries}
							min="0"
							max="3"
							required
						/></label
					>
				{/if}
				<div class="form-row">
					<label>Failures before incident<input type="number" min="1" max="10" bind:value={failures} /></label
					><label
						>Successes before recovery<input type="number" min="1" max="10" bind:value={recovery} /></label
					>
				</div>
				<div class="form-row">
					<label>Group<input bind:value={group} maxlength="120" /></label><label
						>Tags, comma separated<input bind:value={tags} /></label
					>
				</div>
				<label>Description<textarea bind:value={description} maxlength="2000"></textarea></label><label
					class="checkbox"
					><input type="checkbox" bind:checked={publicMonitor} /> Allow inclusion on public status pages</label
				>
				<p class="muted small-text">
					Public pages expose selected names, states and availability, never targets or credentials.
					Notification channels choose their own routes and events.
				</p>
			</div>
		</details>
		{#if error}<div class="error" role="alert">{error}</div>{/if}{#if test}<div class="notice" role="status">
				{test}
			</div>{/if}
		<div class="form-actions">
			{#if !agent && kind !== 'push'}<button
					type="button"
					class="button"
					disabled={busy}
					onclick={() => void testConnection()}>Test connection</button
				>{/if}<button class="button primary" disabled={busy}
				>{busy ? 'Working…' : monitor?.id ? 'Save changes' : 'Create monitor'}</button
			>
		</div>
	</form>{/if}
