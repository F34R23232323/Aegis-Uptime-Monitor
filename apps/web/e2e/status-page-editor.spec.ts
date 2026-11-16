import { test, expect } from '@playwright/test';

test('an existing status page opens and saves without reactive proxy or pattern errors', async ({ page }) => {
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	page.on('console', (message) => {
		if (message.type() === 'error' && message.text().includes('not a valid regexp'))
			errors.push(message.text());
	});
	const statusPage = {
		id: 'a45798ba-56b6-498d-970c-285e782a7190',
		title: 'Service status',
		slug: 'service-status',
		description: 'Current availability',
		accent: '#9a85ff',
		theme: 'dark',
		logo_url: '',
		support_url: '',
		announcement: '',
		resource_ids: [],
		monitor_ids: [],
		tabs: [{ id: 'overview', label: 'Overview', kind: 'overview', body: '' }]
	};
	let saved: Record<string, unknown> | null = null;
	await page.route('**/api/v1/**', async (route) => {
		const path = new URL(route.request().url()).pathname;
		if (path === `/api/v1/status-pages/${statusPage.id}` && route.request().method() === 'PUT') {
			saved = route.request().postDataJSON();
			Object.assign(statusPage, saved);
			await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(statusPage) });
			return;
		}
		const body =
			path === '/api/v1/setup'
				? { needs_setup: false, requires_setup_token: false, name: 'Aegis Monitor', accent: '#a18aff' }
				: path === '/api/v1/me'
					? { id: 'owner', email: 'owner@example.test', role: 'owner' }
					: path === '/api/v1/status-pages'
						? [statusPage]
						: [];
		await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });
	});
	await page.goto('/status-pages');
	await page.getByRole('button', { name: 'Edit', exact: true }).click();
	const dialog = page.getByRole('dialog', { name: 'Edit status page' });
	await expect(dialog).toBeVisible();
	const slug = dialog.getByRole('textbox', { name: 'Slug' });
	await expect(slug).toHaveValue('service-status');
	await expect(slug.evaluate((input: HTMLInputElement) => input.checkValidity())).resolves.toBe(true);
	const tabId = dialog.getByRole('textbox', { name: 'Tab ID (URL hash)' });
	await expect(tabId.evaluate((input: HTMLInputElement) => input.checkValidity())).resolves.toBe(true);
	await dialog.getByRole('textbox', { name: 'Page title' }).fill('Service status updated');
	await dialog.getByLabel('Page layout').selectOption('provider');
	await dialog.getByLabel('Show response-time graph').uncheck();
	await dialog.getByLabel('Response samples').selectOption('60');
	await dialog.getByRole('button', { name: 'Save status page' }).click();
	await expect(dialog).toBeHidden();
	await expect(page.getByRole('heading', { name: 'Service status updated' }).first()).toBeVisible();
	await expect.poll(() => saved?.title).toBe('Service status updated');
	await expect.poll(() => saved?.layout).toBe('provider');
	await expect.poll(() => (saved?.metrics_settings as { show_latency: boolean })?.show_latency).toBe(false);
	await expect.poll(() => (saved?.metrics_settings as { latency_samples: number })?.latency_samples).toBe(60);
	await expect.poll(() => errors).toEqual([]);
});
