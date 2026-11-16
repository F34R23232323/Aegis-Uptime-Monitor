import { test, expect } from '@playwright/test';

test('PM2 editor select is enabled and accepts a discovered app', async ({ page }) => {
	const monitor = {
		id: 'a45798ba-56b6-498d-970c-285e782a7190',
		name: 'test',
		kind: 'pm2',
		target: '/var/lib/appsvc/.pm2:1',
		server_id: '7683f256-bcee-479b-b4fc-577ca3a68b54',
		state: 'UP',
		enabled: true,
		interval_seconds: 30,
		timeout_seconds: 10,
		failure_threshold: 2,
		recovery_threshold: 2,
		group_name: '',
		description: '',
		tags: [],
		public: false,
		config: {},
		last_latency: 0,
		last_error: null,
		last_checked_at: new Date().toISOString()
	};
	const server = {
		id: monitor.server_id,
		name: 'hetzner',
		last_seen: new Date().toISOString(),
		version: '0.1.0',
		inventory: {
			pm2: [
				{ id: monitor.target, name: 'sunstore', status: 'online', healthy: true },
				{ id: '/var/lib/appsvc/.pm2:0', name: 'sunlinks', status: 'online', healthy: true }
			],
			docker: [],
			pm2_errors: []
		}
	};
	await page.route('**/api/v1/**', async (route) => {
		const path = new URL(route.request().url()).pathname;
		const body =
			path === '/api/v1/setup'
				? { needs_setup: false, requires_setup_token: false, name: 'Aegis Monitor', accent: '#a18aff' }
				: path === '/api/v1/me'
					? { id: 'owner', email: 'owner@example.test', role: 'owner' }
					: path === '/api/v1/monitors/counts'
						? { total: 1, states: { UP: 1 }, open_incidents: 0 }
						: path === '/api/v1/monitors/summaries'
							? [
									{
										id: monitor.id,
										checks: [],
										uptime: { days: 1, uptime: 100, covered_seconds: 60, up_seconds: 60, down_seconds: 0 }
									}
								]
							: path.startsWith('/api/v1/monitors/') && path.endsWith('/checks')
								? []
								: path.startsWith('/api/v1/monitors/') && path.endsWith('/uptime')
									? { days: 1, uptime: 100, covered_seconds: 60, up_seconds: 60, down_seconds: 0 }
									: path === '/api/v1/monitors'
										? [monitor]
										: path === '/api/v1/servers'
											? [server]
											: [];
		await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });
	});
	await page.goto('/monitors/' + monitor.id);
	await page.getByRole('button', { name: 'Edit monitor' }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByText('Monitor type', { exact: true })).toBeVisible();
	await expect(dialog.getByRole('combobox')).toHaveCount(2);
	const select = dialog.getByRole('combobox').nth(1);
	await expect(select).toBeVisible();
	await expect(select).toBeEnabled();
	await select.click();
	await select.selectOption('/var/lib/appsvc/.pm2:0');
	await expect(select).toHaveValue('/var/lib/appsvc/.pm2:0');
	await expect(dialog).not.toContainText('> PM2 application');
});
