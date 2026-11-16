<script lang="ts">
	import { ChevronDown, FolderClosed, Radio } from '@lucide/svelte';
	import { monitorStatus, type Monitor } from './api';
	import Status from './Status.svelte';

	let { monitors, search = '' }: { monitors: Monitor[]; search?: string } = $props();
	let folders = $derived.by(() => {
		const grouped = new Map<string, Monitor[]>();
		for (const monitor of monitors) {
			const name = monitor.group_name || 'Premade monitors';
			grouped.set(name, [...(grouped.get(name) || []), monitor]);
		}
		return [...grouped].sort(([a], [b]) => a.localeCompare(b));
	});
	function folderState(items: Monitor[]) {
		if (items.some((m) => m.state === 'DOWN')) return 'DOWN';
		if (items.some((m) => m.state === 'DEGRADED')) return 'DEGRADED';
		if (items.every((m) => m.state === 'PAUSED')) return 'PAUSED';
		if (items.some((m) => ['UNKNOWN', 'PENDING', 'MAINTENANCE', 'PAUSED'].includes(m.state)))
			return 'UNKNOWN';
		return 'UP';
	}
</script>

{#if folders.length}
	<div class="preset-folder-section">
		<div class="preset-folder-heading">
			<FolderClosed size={16} /><strong>Premade monitor folders</strong><span>{folders.length}</span>
		</div>
		<div class="preset-folder-grid">
			{#each folders as [name, items] (name)}
				<details class="preset-folder" open={Boolean(search.trim())}>
					<summary>
						<span class="folder-emblem"><FolderClosed size={20} /></span>
						<span class="folder-title"
							><strong>{name}</strong><small
								>{items.length} component monitor{items.length === 1 ? '' : 's'}</small
							></span
						>
						<Status state={folderState(items)} small />
						<span class="folder-chevron"><ChevronDown size={16} /></span>
					</summary>
					<div class="folder-contents">
						{#each items as monitor (monitor.id)}<a href={'/monitors/' + monitor.id} class="folder-monitor">
								<span class="folder-monitor-name"
									><Radio size={13} /><span>{monitor.name.replace(`${name} · `, '')}</span></span
								>
								<Status {...monitorStatus(monitor)} small />
							</a>{/each}
					</div>
				</details>
			{/each}
		</div>
	</div>
{/if}

<style>
	.preset-folder-section {
		padding: 18px 20px 22px;
		border-bottom: 1px solid var(--border);
	}
	.preset-folder-heading {
		display: flex;
		align-items: center;
		gap: 9px;
		margin-bottom: 13px;
		color: #cbccda;
	}
	.preset-folder-heading span {
		color: var(--muted);
		font-size: 11px;
	}
	.preset-folder-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 340px), 1fr));
		gap: 11px;
	}
	.preset-folder {
		min-width: 0;
		border: 1px solid #30313c;
		background: #171820;
		border-radius: 10px;
		overflow: hidden;
	}
	.preset-folder[open] {
		border-color: #484357;
	}
	.preset-folder summary {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 15px 16px;
		cursor: pointer;
		list-style: none;
	}
	.preset-folder summary::-webkit-details-marker {
		display: none;
	}
	.preset-folder summary:hover {
		background: #1d1e29;
	}
	.folder-emblem {
		width: 38px;
		height: 38px;
		display: grid;
		place-items: center;
		flex: none;
		border-radius: 9px;
		background: #a18aff20;
		color: var(--accent);
	}
	.folder-title {
		display: flex;
		flex: 1;
		min-width: 0;
		flex-direction: column;
		gap: 2px;
	}
	.folder-title strong {
		font-size: 14px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.folder-title small {
		color: var(--muted);
		font-size: 11px;
	}
	.folder-chevron {
		color: var(--muted);
		transition: transform 0.2s;
	}
	.preset-folder[open] .folder-chevron {
		transform: rotate(180deg);
	}
	.folder-contents {
		max-height: 340px;
		overflow: auto;
		border-top: 1px solid var(--border);
	}
	.folder-monitor {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding: 11px 16px;
		border-top: 1px solid #252630;
		font-size: 12px;
	}
	.folder-monitor:first-child {
		border-top: 0;
	}
	.folder-monitor:hover {
		background: #20212b;
	}
	.folder-monitor-name {
		display: flex;
		align-items: center;
		gap: 9px;
		min-width: 0;
	}
	.folder-monitor-name :global(svg) {
		color: var(--muted);
	}
	.folder-monitor-name span {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	@media (max-width: 600px) {
		.preset-folder-section {
			padding: 15px;
		}
		.preset-folder summary {
			gap: 9px;
			padding: 12px;
		}
		.folder-emblem {
			width: 32px;
			height: 32px;
		}
	}
</style>
