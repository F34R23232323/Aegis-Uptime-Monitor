// Capture real Aegis views with synthetic data. Requires Vite on 127.0.0.1:5173.
import { chromium } from '@playwright/test';
import { createServer } from 'node:http';
import { mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import {
	monitors,
	summaries,
	incidents,
	servers,
	resources,
	page as statusPage,
	maintenance,
	channels,
	deliveries,
	users,
	sessions,
	audit,
	publicPage,
	providerSummary,
	responseFor
} from './readme-fixtures.mjs';

const baseURL = 'http://127.0.0.1:5173';
const output = resolve('../../docs/screenshots');
const files = [];

const backend = createServer((request, response) => {
	if (request.url === '/api/v1/public/status/demo') {
		response.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store' });
		response.end(JSON.stringify(publicPage));
		return;
	}
	response.writeHead(404, { 'content-type': 'application/json' });
	response.end(JSON.stringify({ error: 'Synthetic fixture route not found' }));
});

async function mockAPI(browserPage, { setup = false, guest = false } = {}) {
	await browserPage.addInitScript(() => {
		window.EventSource = class {
			constructor() {
				setTimeout(() => this.onopen?.(), 0);
			}
			close() {}
		};
	});
	await browserPage.route('**/provider-status/*', (route) =>
		route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(providerSummary) })
	);
	await browserPage.route('**/api/v1/**', (route) => {
		const url = new URL(route.request().url());
		const path = url.pathname.replace('/api/v1', '');
		if (path === '/events') return route.abort();
		if (path === '/me' && guest) {
			return route.fulfill({
				status: 401,
				contentType: 'application/json',
				body: '{"error":"Sign in required"}'
			});
		}
		let data =
			path === '/setup' && setup
				? { needs_setup: true, requires_setup_token: false, name: 'Aegis Monitor', accent: '#a18aff' }
				: responseFor(path, route.request().method());
		if (path.endsWith('/uptime')) {
			const days = Number(url.searchParams.get('days') || 1);
			data = { ...data, days, covered_seconds: days * 86_400, up_seconds: days * 86_400, down_seconds: 0 };
		}
		return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(data) });
	});
}

async function visit(browserPage, path, expectedHeading) {
	await browserPage.goto(baseURL + path, { waitUntil: 'networkidle' });
	await browserPage.getByRole('heading', { name: expectedHeading, exact: false }).first().waitFor();
	if (await browserPage.getByRole('heading', { name: 'Connection interrupted' }).count())
		throw new Error('App failed to load at ' + path);
}
async function shot(browserPage, name) {
	await browserPage.screenshot({ path: resolve(output, name + '.png'), fullPage: true });
	files.push(name + '.png');
	console.log('Captured ' + name);
}
async function dialogShot(browserPage, name, focusText) {
	const dialog = browserPage.getByRole('dialog');
	await dialog.waitFor();
	if (focusText) await dialog.getByText(focusText, { exact: false }).first().scrollIntoViewIfNeeded();
	await browserPage.waitForTimeout(350); // Let the dialog's entrance transition finish before capture.
	await shot(browserPage, name);
}
async function closeDialog(browserPage) {
	await browserPage.getByRole('dialog').getByRole('button', { name: 'Close' }).click();
	await browserPage.getByRole('dialog').waitFor({ state: 'hidden' });
}

await mkdir(output, { recursive: true });
await new Promise((done, fail) => backend.once('error', fail).listen(3001, '127.0.0.1', done));
const browser = await chromium.launch({
	headless: true,
	...(process.env.PLAYWRIGHT_CHROME_EXECUTABLE
		? { executablePath: process.env.PLAYWRIGHT_CHROME_EXECUTABLE }
		: {})
});
try {
	const browserPage = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
	await mockAPI(browserPage);

	await visit(browserPage, '/', 'Infrastructure, in harmony.');
	await shot(browserPage, 'dashboard');
	await visit(browserPage, '/monitors', 'Your monitors.');
	await shot(browserPage, 'monitors');
	await browserPage.getByRole('button', { name: 'Add monitor', exact: true }).click();
	await dialogShot(browserPage, 'monitor-create');
	await browserPage.getByRole('dialog').getByText('Organization & advanced settings').click();
	await dialogShot(browserPage, 'monitor-advanced', 'Raw check retention');
	await closeDialog(browserPage);
	await browserPage.getByRole('button', { name: 'Add monitor', exact: true }).click();
	await browserPage.getByRole('dialog').getByLabel('Monitor type').selectOption('json');
	await dialogShot(browserPage, 'monitor-json', 'JSON Pointer');
	await closeDialog(browserPage);

	await visit(browserPage, '/monitors/' + monitors[0].id, 'Public website');
	await browserPage.getByRole('heading', { name: 'Response latency' }).waitFor();
	await shot(browserPage, 'monitor-detail');
	await browserPage.getByRole('button', { name: 'Edit monitor' }).click();
	await dialogShot(browserPage, 'monitor-edit');
	await browserPage.getByRole('dialog').getByText('Encrypted authentication & headers').click();
	await dialogShot(browserPage, 'monitor-credentials', 'Credentials');
	await closeDialog(browserPage);

	await visit(browserPage, '/presets', 'Watch the services behind yours.');
	await shot(browserPage, 'presets');
	await browserPage
		.getByRole('button', { name: /Cloudflare/ })
		.first()
		.click();
	await browserPage.getByRole('heading', { name: 'Published components' }).waitFor();
	await shot(browserPage, 'preset-components');

	await visit(browserPage, '/servers', 'Your infrastructure.');
	await shot(browserPage, 'servers');
	await browserPage.getByRole('button', { name: 'Expand worker-01' }).click();
	await browserPage.getByRole('heading', { name: 'Discovered services' }).waitFor();
	await shot(browserPage, 'server-detail');
	await browserPage.getByRole('button', { name: 'Connect server' }).click();
	await dialogShot(browserPage, 'server-connect');
	await closeDialog(browserPage);

	await visit(browserPage, '/incidents', 'Incident journal.');
	await shot(browserPage, 'incidents');
	await browserPage.getByRole('button', { name: 'Show timeline' }).first().click();
	await browserPage.getByText('We are reviewing the worker queue.').waitFor();
	await shot(browserPage, 'incident-timeline');
	await browserPage.getByRole('button', { name: 'Edit incident' }).first().click();
	await dialogShot(browserPage, 'incident-edit');
	await closeDialog(browserPage);
	await browserPage.getByRole('button', { name: 'Post update' }).first().click();
	await dialogShot(browserPage, 'incident-update');
	await closeDialog(browserPage);

	await visit(browserPage, '/analytics', 'Reliability, measured.');
	await shot(browserPage, 'analytics');

	await visit(browserPage, '/notifications', 'Stay in the loop.');
	await browserPage.getByRole('heading', { name: 'Operations Discord' }).waitFor();
	await shot(browserPage, 'notifications');
	await browserPage.getByRole('button', { name: 'Edit', exact: true }).first().click();
	await dialogShot(browserPage, 'notification-edit');
	await dialogShot(browserPage, 'notification-designer', 'Embed fields');
	await closeDialog(browserPage);

	await visit(browserPage, '/status-pages', 'Share the bigger picture.');
	await browserPage.getByRole('heading', { name: 'Aegis Demo Status' }).first().waitFor();
	await shot(browserPage, 'status-pages');
	await browserPage.getByRole('button', { name: 'Edit', exact: true }).first().click();
	await dialogShot(browserPage, 'status-page-edit');
	await dialogShot(browserPage, 'status-page-tabs', 'Public page tabs');
	await dialogShot(browserPage, 'status-page-graphs', 'Public graphs');
	await closeDialog(browserPage);
	await browserPage.getByRole('tab', { name: /Resources/ }).click();
	await shot(browserPage, 'status-resources');
	await browserPage.getByRole('button', { name: 'Edit status' }).first().click();
	await dialogShot(browserPage, 'resource-edit');
	await closeDialog(browserPage);

	await visit(browserPage, '/maintenance', 'Planned, not unexpected.');
	await shot(browserPage, 'maintenance');
	await browserPage.getByRole('button', { name: 'Edit', exact: true }).first().click();
	await dialogShot(browserPage, 'maintenance-edit');
	await dialogShot(browserPage, 'maintenance-publication', 'Public status pages');
	await closeDialog(browserPage);

	await visit(browserPage, '/settings', 'Instance settings.');
	await browserPage.getByRole('heading', { name: 'Active sessions' }).waitFor();
	await shot(browserPage, 'settings');

	await visit(browserPage, '/status/demo', 'Aegis Demo Status');
	await shot(browserPage, 'public-overview');
	for (const [tab, name] of [
		['Incidents', 'public-incidents'],
		['Maintenance', 'public-maintenance'],
		['Metrics', 'public-metrics'],
		['Support', 'public-support']
	]) {
		await browserPage
			.getByRole('navigation', { name: 'Public status sections' })
			.getByRole('link', { name: tab, exact: false })
			.click();
		await shot(browserPage, name);
	}
	await browserPage.close();

	const setupPage = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
	await mockAPI(setupPage, { setup: true });
	await visit(setupPage, '/', 'Your command center awaits.');
	await shot(setupPage, 'setup');
	await setupPage.close();

	const signInPage = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
	await mockAPI(signInPage, { guest: true });
	await visit(signInPage, '/', 'Good to see you.');
	await shot(signInPage, 'sign-in');
	await signInPage.close();
} finally {
	await browser.close();
	await new Promise((done) => backend.close(done));
}
console.log('Captured ' + files.length + ' documentation screenshots.');
