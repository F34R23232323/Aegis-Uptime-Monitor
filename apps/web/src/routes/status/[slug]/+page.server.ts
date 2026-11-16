import { error } from '@sveltejs/kit';
import type { PublicPage } from '#lib/public.ts';
export async function load({
	params,
	fetch,
	setHeaders
}: {
	params: { slug: string };
	fetch: typeof globalThis.fetch;
	setHeaders: (headers: Record<string, string>) => void;
}) {
	setHeaders({ 'cache-control': 'no-store' });
	const backend = (process.env.AEGIS_INTERNAL_API_URL || 'http://127.0.0.1:3001').replace(/\/$/, '');
	let response: Response;
	try {
		response = await fetch(backend + '/api/v1/public/status/' + encodeURIComponent(params.slug), {
			signal: AbortSignal.timeout(10000),
			cache: 'no-store'
		});
	} catch {
		error(503, 'The status service is temporarily unavailable. Please try again shortly.');
	}
	if (!response.ok)
		error(
			response.status === 404 ? 404 : 503,
			response.status === 404
				? 'This status page does not exist.'
				: 'The status service is temporarily unavailable.'
		);
	return { status: (await response.json()) as PublicPage };
}
