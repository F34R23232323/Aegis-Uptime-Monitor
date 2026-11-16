<script lang="ts">
	import type { DiscordPayload } from './api';
	let { payload }: { payload: DiscordPayload | null } = $props();
	let embed = $derived(payload?.embeds?.[0]);
</script>

<div class="discord-preview" aria-label="Discord embed preview">
	<div class="discord-author">
		<div class="discord-avatar">A</div>
		<strong>{payload?.username || 'Aegis Monitor'}</strong><span>APP</span><small
			>Preview · no message sent</small
		>
	</div>
	{#if payload?.content}<p class="discord-content">{payload.content}</p>{/if}
	{#if embed}<div
			class="discord-embed"
			style={`border-left-color:#${embed.color.toString(16).padStart(6, '0')}`}
		>
			<h3>{embed.title}</h3>
			<p>{embed.description}</p>
			{#if embed.fields?.length}<div class="discord-fields">
					{#each embed.fields as field}<div class:wide={!field.inline}>
							<strong>{field.name}</strong>
							<p>{field.value}</p>
						</div>{/each}
				</div>{/if}
			<footer>
				{embed.footer?.text}{embed.timestamp ? ' · ' + new Date(embed.timestamp).toLocaleString() : ''}
			</footer>
		</div>{:else}<div class="calm-state">
			<p>Use Preview message to render your template through the server.</p>
		</div>{/if}
</div>
