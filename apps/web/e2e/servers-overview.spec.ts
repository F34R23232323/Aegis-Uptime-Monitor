import { test, expect } from '@playwright/test';

test('server overview keeps long service lists searchable and collapsed', async ({ page }) => {
	const now = new Date().toISOString();
	const servers = [
		{
			id: 'one',
			name: 'primary-host',
			last_seen: now,
			version: '1.0',
			inventory: {
				host: {
					cpu_percent: 32,
					memory_percent: 58,
					memory_used: 6_227_020_800,
					memory_total: 10_737_418_240,
					load: 0.8,
					uptime_seconds: 172800,
					disks: [{ mount: '/', total: 107_374_182_400, available: 53_687_091_200 }],
					network: []
				},
				docker: Array.from({ length: 20 }, (_, index) => ({
					id: `docker-${index}`,
					name: `service-${index}`,
					state: 'running',
					healthy: true,
					restarts: 0
				})),
				pm2: [{ id: 'pm2-1', name: 'api-worker', status: 'online', healthy: true }]
			}
		},
		{ id: 'two', name: 'backup-host', last_seen: null, version: null, inventory: { docker: [], pm2: [] } }
	];
	await page.route('**/api/v1/**', async (route) => {
		const path = new URL(route.request().url()).pathname;
		const body =
			path === '/api/v1/setup'
				? { needs_setup: false, requires_setup_token: false, name: 'Aegis Monitor', accent: '#a18aff' }
				: path === '/api/v1/me'
					? { id: 'owner', email: 'owner@example.test', role: 'owner' }
					: path === '/api/v1/servers'
						? servers
						: path === '/api/v1/monitors/counts'
							? { total: 0, states: {}, open_incidents: 0 }
							: [];
		await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });
	});
	await page.goto('/servers');
	await expect(page.getByText('Connected servers')).toBeVisible();
	await expect(page.getByText('21', { exact: true }).first()).toBeVisible();
	await expect(page.getByText('service-19')).toBeHidden();
	await page.getByRole('button', { name: 'Expand primary-host' }).click();
	await expect(page.getByText('service-19')).toBeVisible();
	await page.getByRole('textbox', { name: 'Search services on primary-host' }).fill('api-worker');
	await expect(page.getByText('api-worker')).toBeVisible();
	await expect(page.getByText('service-19')).toBeHidden();
	await page.getByRole('button', { name: 'Offline' }).click();
	await expect(page.getByRole('heading', { name: 'backup-host' })).toBeVisible();
	await expect(page.getByRole('heading', { name: 'primary-host' })).toBeHidden();
});
