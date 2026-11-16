import { test, expect } from '@playwright/test';
import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';
test('owner → HTTP monitor → outage → durable alert → recovery → public status', async ({
	page,
	request,
	baseURL
}) => {
	const monitorName = 'E2E API ' + Date.now();
	let healthy = true;
	const received: Record<string, unknown>[] = [];
	const target = createServer((_q, r) => {
		r.writeHead(healthy ? 200 : 503, { 'content-type': 'text/plain' });
		r.end(healthy ? 'healthy' : 'outage');
	});
	const hook = createServer((q, r) => {
		let body = '';
		q.on('data', (c) => (body += c));
		q.on('end', () => {
			received.push(JSON.parse(body));
			r.writeHead(204);
			r.end();
		});
	});
	await new Promise<void>((r) => target.listen(19191, '127.0.0.1', r));
	await new Promise<void>((r) => hook.listen(19192, '127.0.0.1', r));
	const errors: string[] = [];
	page.on('pageerror', (e) => errors.push(e.message));
	try {
		await page.goto('/');
		const setup = await request.get('/api/v1/setup').then((r) => r.json());
		const email = process.env.TEST_OWNER_EMAIL || 'e2e@example.test';
		const password = process.env.TEST_OWNER_PASSWORD || 'e2e-owner-password-unique';
		await page.getByLabel('Email address').fill(email);
		await page.getByLabel('Password', { exact: true }).fill(password);
		await page
			.getByRole('button', { name: setup.needs_setup ? 'Create owner account' : 'Sign in', exact: false })
			.click();
		await expect(page.getByRole('heading', { name: 'Infrastructure, in harmony.' })).toBeVisible();
		await page.getByRole('button', { name: 'Add monitor', exact: true }).click();
		const dialog = page.getByRole('dialog');
		await dialog.getByLabel('Name', { exact: true }).fill(monitorName);
		await dialog.getByLabel('URL', { exact: true }).fill('http://127.0.0.1:19191/health');
		await dialog.getByLabel('Check interval').fill('20');
		await dialog.getByText('Organization & advanced settings').click();
		await dialog.getByLabel('Failures before incident').fill('1');
		await dialog.getByLabel('Successes before recovery').fill('1');
		await dialog.getByLabel('Allow inclusion on public status pages').check();
		await dialog.getByRole('button', { name: 'Test connection' }).click();
		await expect(dialog.getByText(/Connection successful/)).toBeVisible();
		await dialog.getByRole('button', { name: 'Create monitor', exact: true }).click();
		await expect(dialog).not.toBeVisible();
		const list = await page.request.get('/api/v1/monitors').then((r) => r.json());
		const monitor = list.find(
			(m: { target: string; name: string }) =>
				m.target === 'http://127.0.0.1:19191/health' && m.name === monitorName
		);
		expect(monitor).toBeTruthy();
		const post = async (path: string, data: unknown) => {
			const r = await page.request.post('/api/v1' + path, { headers: { Origin: baseURL! }, data });
			expect(r.ok(), await r.text()).toBeTruthy();
			return r.json();
		};
		const channel = await post('/channels', {
			name: 'E2E webhook',
			provider: 'webhook',
			secret: 'http://127.0.0.1:19192'
		});
		await post('/channels/' + channel.id + '/test', {});
		await expect.poll(() => received.length, { timeout: 15000 }).toBeGreaterThan(0);
		const state = () =>
			page.request
				.get('/api/v1/monitors/' + monitor.id)
				.then((r) => r.json())
				.then((m) => m.state);
		await expect.poll(state, { timeout: 35000 }).toBe('UP');
		healthy = false;
		await expect.poll(state, { timeout: 35000 }).toBe('DOWN');
		await expect.poll(() => received.some((p) => p.state === 'DOWN'), { timeout: 15000 }).toBeTruthy();
		let incidents = await page.request.get('/api/v1/incidents').then((r) => r.json());
		expect(
			incidents.some(
				(i: { monitor_id: string; ended_at: string | null }) => i.monitor_id === monitor.id && !i.ended_at
			)
		).toBeTruthy();
		healthy = true;
		await expect.poll(state, { timeout: 35000 }).toBe('UP');
		await expect.poll(() => received.some((p) => p.state === 'RECOVERED'), { timeout: 15000 }).toBeTruthy();
		incidents = await page.request.get('/api/v1/incidents').then((r) => r.json());
		expect(
			incidents.some(
				(i: { monitor_id: string; ended_at: string | null }) => i.monitor_id === monitor.id && i.ended_at
			)
		).toBeTruthy();
		const slug = 'e2e-' + Date.now();
		await post('/status-pages', {
			slug,
			title: 'E2E Service Status',
			monitor_ids: [monitor.id],
			description: 'Actual integration test'
		});
		await page.goto('/status/' + slug);
		await expect(page.getByRole('heading', { name: 'All systems operational' })).toBeVisible();
		await expect(page.getByRole('heading', { name: monitor.name, exact: true })).toBeVisible();
		await expect(page.locator('body')).not.toContainText('127.0.0.1');
		await page.goto('/monitors/' + monitor.id);
		await expect(page.getByRole('heading', { name: 'Response latency' })).toBeVisible();
		await page.screenshot({ path: '/tmp/aegis-detail.png', fullPage: true });
		await page.setViewportSize({ width: 390, height: 844 });
		await page.goto('/');
		await expect(page.getByRole('heading', { name: 'Infrastructure, in harmony.' })).toBeVisible();
		expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBeTruthy();
		await page.screenshot({ path: '/tmp/aegis-mobile.png', fullPage: true });
		expect(errors).toEqual([]);
		await page.request.delete('/api/v1/channels/' + channel.id, { headers: { Origin: baseURL! } });
	} finally {
		target.closeAllConnections();
		await new Promise<void>((r) => target.close(() => r()));
		hook.closeAllConnections();
		await new Promise<void>((r) => hook.close(() => r()));
	}
});
