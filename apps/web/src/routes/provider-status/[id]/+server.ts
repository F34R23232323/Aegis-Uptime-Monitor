import { json } from '@sveltejs/kit';
import { presets, type PresetSummary } from '../../../lib/presets';
import type { RequestHandler } from './$types';

interface GoogleProduct {
	id: string;
	title: string;
	current_title?: string;
}
interface GoogleIncident {
	id: string;
	external_desc: string;
	end?: string | null;
	modified: string;
	status_impact: string;
	affected_products?: { id: string }[];
}

export const GET: RequestHandler = async ({ params, fetch }) => {
	const preset = presets.find((p) => p.id === params.id);
	if (!preset) return json({ error: 'Unknown provider' }, { status: 404 });
	try {
		if (preset.id === 'google-cloud') {
			const [productsResponse, incidentsResponse] = await Promise.all([
				fetch(`${preset.origin}/products.json`, { signal: AbortSignal.timeout(10000) }),
				fetch(`${preset.origin}/incidents.json`, { signal: AbortSignal.timeout(10000) })
			]);
			if (!productsResponse.ok || !incidentsResponse.ok) throw new Error('Google Cloud feed unavailable');
			const products = (await productsResponse.json()) as { products: GoogleProduct[] };
			const incidents = (await incidentsResponse.json()) as GoogleIncident[];
			if (!Array.isArray(products.products) || !Array.isArray(incidents))
				throw new Error('Invalid Google Cloud feed');
			const active = incidents.filter(
				(i) => !i.end && ['SERVICE_OUTAGE', 'SERVICE_DISRUPTION'].includes(i.status_impact)
			);
			const affected = new Map<string, string>();
			for (const incident of active)
				for (const product of incident.affected_products || [])
					affected.set(
						product.id,
						incident.status_impact === 'SERVICE_OUTAGE' ? 'major_outage' : 'degraded_performance'
					);
			return json(
				{
					page: { updated_at: active[0]?.modified },
					status: {
						description: active.length
							? `${active.length} active public incident${active.length === 1 ? '' : 's'}`
							: 'No active public incidents',
						indicator: active.length ? 'major' : 'none'
					},
					components: products.products.map((p) => ({
						id: p.id,
						name: p.current_title || p.title,
						status: affected.get(p.id) || 'operational',
						group: false,
						group_id: null,
						description: null
					})),
					incidents: active.map((i) => ({
						id: i.id,
						name: i.external_desc,
						status: i.status_impact,
						updated_at: i.modified
					}))
				},
				{ headers: { 'cache-control': 'private, max-age=30' } }
			);
		}
		const response = await fetch(`${preset.origin}/api/v2/summary.json`, {
			headers: { accept: 'application/json', 'user-agent': 'AegisMonitor/1.0 (+https://github.com/)' },
			signal: AbortSignal.timeout(10000)
		});
		if (!response.ok) return json({ error: 'Provider status API is unavailable' }, { status: 502 });
		const raw = (await response.json()) as PresetSummary;
		if (!Array.isArray(raw.components) || !raw.status || !Array.isArray(raw.incidents))
			return json({ error: 'Provider returned an unexpected response' }, { status: 502 });
		return json(
			{
				page: { updated_at: raw.page?.updated_at },
				status: raw.status,
				components: raw.components.map(({ id, name, status, group, group_id, description }) => ({
					id,
					name,
					status,
					group,
					group_id,
					description
				})),
				incidents: raw.incidents.map(({ id, name, status, updated_at }) => ({ id, name, status, updated_at }))
			},
			{ headers: { 'cache-control': 'private, max-age=30' } }
		);
	} catch {
		return json({ error: 'Could not reach provider status API' }, { status: 502 });
	}
};
