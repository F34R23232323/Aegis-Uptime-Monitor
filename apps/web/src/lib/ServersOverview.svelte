<script lang="ts">
	import {
		Activity,
		Check,
		ChevronDown,
		Container,
		HardDrive,
		Layers,
		Plus,
		RefreshCw,
		Search,
		Server as ServerIcon,
		Trash2
	} from '@lucide/svelte';
	import Status from './Status.svelte';
	import { relative, type Monitor, type Server, type Service } from './api';

	let {
		servers,
		monitors,
		readonly,
		ondiscover,
		onrotate,
		onrevoke
	}: {
		servers: Server[];
		monitors: Monitor[];
		readonly: boolean;
		ondiscover: (server: Server, item?: Service, kind?: string) => void;
		onrotate: (server: Server) => void;
		onrevoke: (server: Server) => void;
	} = $props();

	let query = $state('');
	let stateFilter = $state<'all' | 'online' | 'offline'>('all');
	let sort = $state<'name' | 'cpu' | 'memory'>('name');
	let expanded = $state<Record<string, boolean>>({});
	let serviceQueries = $state<Record<string, string>>({});
	let serviceTabs = $state<Record<string, 'all' | 'docker' | 'pm2'>>({});

	const online = (server: Server) =>
		!!server.last_seen && Date.now() - new Date(server.last_seen).getTime() < 60_000;
	const services = (server: Server) => [
		...(server.inventory.docker || []).map((item) => ({ item, kind: 'docker' as const })),
		...(server.inventory.pm2 || []).map((item) => ({ item, kind: 'pm2' as const }))
	];
	const monitored = (server: Server, item: Service) =>
		monitors.some((monitor) => monitor.server_id === server.id && monitor.target === item.id);
	const usage = (value: number) => Math.min(100, Math.max(0, Number.isFinite(value) ? value : 0));
	const gb = (value: number) => (value / 1073741824).toFixed(1);
	let onlineCount = $derived(servers.filter(online).length);
	let allServices = $derived(servers.flatMap(services));
	let healthyCount = $derived(allServices.filter(({ item }) => item.healthy).length);
	let monitoredCount = $derived(
		servers.reduce(
			(count, server) => count + services(server).filter(({ item }) => monitored(server, item)).length,
			0
		)
	);
	let filteredServers = $derived.by(() => {
		const needle = query.trim().toLowerCase();
		return servers
			.filter(
				(server) =>
					(stateFilter === 'all' || (stateFilter === 'online') === online(server)) &&
					(!needle ||
						server.name.toLowerCase().includes(needle) ||
						services(server).some(({ item }) => item.name.toLowerCase().includes(needle)))
			)
			.sort((a, b) =>
				sort === 'name'
					? a.name.localeCompare(b.name)
					: (b.inventory.host?.[sort === 'cpu' ? 'cpu_percent' : 'memory_percent'] || 0) -
						(a.inventory.host?.[sort === 'cpu' ? 'cpu_percent' : 'memory_percent'] || 0)
			);
	});
</script>

{#if servers.length}
	<section class="fleet-overview" aria-label="Infrastructure overview">
		<div class="fleet-summary">
			<div class="fleet-summary-heading">
				<span>FLEET AT A GLANCE</span><small>Live agent snapshots</small>
			</div>
			<div class="fleet-stats">
				<div>
					<span>Connected servers</span><strong>{onlineCount}<small> / {servers.length}</small></strong><span
						class="fleet-stat-foot">{servers.length - onlineCount} awaiting heartbeat</span
					>
				</div>
				<div>
					<span>Discovered services</span><strong>{allServices.length}</strong><span class="fleet-stat-foot"
						>{monitoredCount} monitored</span
					>
				</div>
				<div>
					<span>Healthy services</span><strong>{healthyCount}<small> / {allServices.length}</small></strong
					><span class="fleet-stat-foot">Docker + PM2</span>
				</div>
			</div>
		</div>
		<div class="fleet-chart-card">
			<div class="fleet-summary-heading"><span>SERVER LOAD</span><small>Current usage</small></div>
			<div class="fleet-chart-legend">
				<span><i class="cpu-key"></i> CPU</span><span><i class="memory-key"></i> Memory</span>
			</div>
			<div class="fleet-chart-list">
				{#each [...servers]
					.sort((a, b) => (b.inventory.host?.cpu_percent || 0) - (a.inventory.host?.cpu_percent || 0))
					.slice(0, 6) as server}
					<div class="fleet-chart-row">
						<span title={server.name}>{server.name}</span>
						<div
							class="fleet-chart-tracks"
							aria-label={`${server.name}: CPU ${server.inventory.host?.cpu_percent?.toFixed(1) ?? 'unknown'}%, memory ${server.inventory.host?.memory_percent?.toFixed(1) ?? 'unknown'}%`}
						>
							<i class="cpu-fill" style={`width:${usage(server.inventory.host?.cpu_percent || 0)}%`}></i><i
								class="memory-fill"
								style={`width:${usage(server.inventory.host?.memory_percent || 0)}%`}
							></i>
						</div>
						<strong>{server.inventory.host ? `${server.inventory.host.cpu_percent.toFixed(0)}%` : '—'}</strong
						>
					</div>
				{/each}
			</div>
			{#if servers.length > 6}<small class="fleet-chart-note">Showing 6 highest CPU servers</small>{/if}
		</div>
	</section>
	<div class="fleet-toolbar">
		<div class="fleet-search">
			<Search size={16} /><input
				aria-label="Search servers and services"
				placeholder="Search servers or services"
				bind:value={query}
			/>
		</div>
		<div class="fleet-filters" aria-label="Server status filter">
			{#each ['all', 'online', 'offline'] as option}<button
					class:active={stateFilter === option}
					onclick={() => (stateFilter = option as typeof stateFilter)}
					>{option === 'all' ? 'All' : option === 'online' ? 'Online' : 'Offline'}</button
				>{/each}
		</div>
		<select aria-label="Sort servers" bind:value={sort}
			><option value="name">Name A–Z</option><option value="cpu">Highest CPU</option><option value="memory"
				>Highest memory</option
			></select
		>
	</div>
	<div class="fleet-list-heading">
		<strong>Servers <span>{filteredServers.length}</span></strong><small
			>Expand a server to inspect disks and services</small
		>
	</div>
	{#if !filteredServers.length}<div class="panel fleet-no-results">
			No servers match this search or filter.
		</div>{/if}
	<div class="fleet-server-list">
		{#each filteredServers as server (server.id)}
			{@const host = server.inventory.host}
			{@const serverServices = services(server)}
			<section class="panel fleet-server">
				<div class="fleet-server-main">
					<button
						class="fleet-server-toggle"
						aria-label={`${expanded[server.id] ? 'Collapse' : 'Expand'} ${server.name}`}
						aria-expanded={!!expanded[server.id]}
						onclick={() => (expanded[server.id] = !expanded[server.id])}
						><ChevronDown size={17} class={expanded[server.id] ? 'turned' : ''} /></button
					>
					<div class="fleet-server-icon"><ServerIcon size={19} /></div>
					<div class="fleet-server-identity">
						<div>
							<h2>{server.name}</h2>
							<Status state={online(server) ? 'UP' : 'UNKNOWN'} small />
						</div>
						<small
							>Last seen {relative(server.last_seen)} · {server.version
								? `Agent v${server.version}`
								: 'Not connected'}</small
						>
					</div>
					<div class="fleet-server-resources">
						<div class="fleet-resource">
							<div><span>CPU</span><strong>{host ? `${host.cpu_percent.toFixed(1)}%` : '—'}</strong></div>
							<div class="fleet-resource-track">
								<i style={`width:${usage(host?.cpu_percent || 0)}%`}></i>
							</div>
						</div>
						<div class="fleet-resource memory">
							<div>
								<span>Memory</span><strong>{host ? `${host.memory_percent.toFixed(1)}%` : '—'}</strong>
							</div>
							<div class="fleet-resource-track">
								<i style={`width:${usage(host?.memory_percent || 0)}%`}></i>
							</div>
						</div>
					</div>
					<div class="fleet-server-counts">
						<span><Container size={13} /> {server.inventory.docker?.length || 0}</span><span
							><Layers size={13} /> {server.inventory.pm2?.length || 0}</span
						>
					</div>
				</div>
				{#if expanded[server.id]}
					<div class="fleet-server-detail">
						{#if host}
							<div class="fleet-host-stats">
								<span>Load <strong>{host.load.toFixed(2)}</strong></span><span
									>Memory <strong>{gb(host.memory_used)} / {gb(host.memory_total)} GB</strong></span
								><span
									>Uptime <strong
										>{Math.floor(host.uptime_seconds / 86400)}d {Math.floor(
											(host.uptime_seconds % 86400) / 3600
										)}h</strong
									></span
								>
							</div>
							{#if host.disks?.length}<div class="fleet-disks">
									<h3><HardDrive size={15} /> Disks</h3>
									<div class="fleet-disk-grid">
										{#each host.disks as disk}<div class="fleet-disk">
												<div>
													<strong title={disk.mount}>{disk.mount}</strong><span
														>{((100 * (disk.total - disk.available)) / Math.max(1, disk.total)).toFixed(0)}%
														used</span
													>
												</div>
												<div class="fleet-resource-track">
													<i
														style={`width:${usage((100 * (disk.total - disk.available)) / Math.max(1, disk.total))}%`}
													></i>
												</div>
												<small>{gb(disk.available)} GB free of {gb(disk.total)} GB</small>
											</div>{/each}
									</div>
								</div>{/if}
						{:else}<p class="fleet-waiting">
								Waiting for the agent’s first heartbeat. Use the enrollment credentials to connect.
							</p>{/if}
						<div class="fleet-services-heading">
							<div>
								<h3>Discovered services <span>{serverServices.length}</span></h3>
								<small
									>{serverServices.filter(({ item }) => item.healthy).length} healthy · {serverServices.filter(
										({ item }) => monitored(server, item)
									).length} monitored</small
								>
							</div>
						</div>
						{#if server.inventory.docker_error}<p class="fleet-adapter-error">
								Docker: {server.inventory.docker_error}
							</p>{/if}
						{#each server.inventory.pm2_errors || [] as error}<p class="fleet-adapter-error">
								PM2: {error}
							</p>{/each}
						{#if serverServices.length}
							<div class="fleet-service-toolbar">
								<div class="fleet-search">
									<Search size={14} /><input
										aria-label={`Search services on ${server.name}`}
										placeholder="Filter services"
										value={serviceQueries[server.id] || ''}
										oninput={(event) => (serviceQueries[server.id] = event.currentTarget.value)}
									/>
								</div>
								<div class="fleet-filters">
									{#each ['all', 'docker', 'pm2'] as tab}<button
											class:active={(serviceTabs[server.id] || 'all') === tab}
											onclick={() => (serviceTabs[server.id] = tab as 'all' | 'docker' | 'pm2')}
											>{tab === 'all' ? 'All' : tab === 'docker' ? 'Docker' : 'PM2'}</button
										>{/each}
								</div>
							</div>
							<div class="fleet-service-list">
								{#each serverServices.filter(({ item, kind }) => ((serviceTabs[server.id] || 'all') === 'all' || serviceTabs[server.id] === kind) && item.name
											.toLowerCase()
											.includes((serviceQueries[server.id] || '').toLowerCase())) as { item, kind } (kind + item.id)}
									<div class="fleet-service">
										<span class:healthy={item.healthy} class="service-dot"></span>
										<div class="fleet-service-name">
											<strong>{kind === 'docker' ? item.name.replace(/^\//, '') : item.name}</strong><small
												>{kind === 'docker' ? 'Docker' : 'PM2'} · {item.state ||
													item.status ||
													'Unknown'}{item.health ? ` · ${item.health}` : ''} · {item.restarts || 0} restarts</small
											>
										</div>
										{#if monitored(server, item)}<span class="fleet-monitored"
												><Check size={13} /> Monitored</span
											>{:else if !readonly}<button
												class="button small"
												onclick={() => ondiscover(server, item, kind)}><Plus size={13} /> Monitor</button
											>{/if}
									</div>
								{:else}<p class="fleet-no-services">No services match this filter.</p>{/each}
							</div>
						{:else}<p class="fleet-no-services">
								No Docker containers or PM2 applications discovered yet.
							</p>{/if}
						{#if !readonly}<div class="fleet-actions">
								<button class="button small" onclick={() => ondiscover(server)}
									><Activity size={14} /> Monitor host health</button
								><button class="button small" onclick={() => onrotate(server)}
									><RefreshCw size={14} /> Rotate token</button
								><button class="button small fleet-danger" onclick={() => onrevoke(server)}
									><Trash2 size={14} /> Revoke server</button
								>
							</div>{/if}
					</div>
				{/if}
			</section>
		{/each}
	</div>
{/if}
