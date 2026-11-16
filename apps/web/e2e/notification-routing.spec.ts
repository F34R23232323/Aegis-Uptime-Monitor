import { test, expect } from '@playwright/test';

test('creates a channel scoped to one monitor with its own events', async ({ page }) => {
	const monitorA = { id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', name: 'API', kind: 'http', group_name: 'Core' };
	const monitorB = { id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', name: 'Website', kind: 'http', group_name: 'Core' };
	let saved: Record<string, unknown> | null = null;
	await page.route('**/api/v1/**', async (route) => {
		const path = new URL(route.request().url()).pathname;
		if (path === '/api/v1/channels/preview') {
			await route.fulfill({ status: 200, contentType: 'application/json', body: '{}' });
			return;
		}
		if (path === '/api/v1/channels' && route.request().method() === 'POST') {
			saved = route.request().postDataJSON();
			await route.fulfill({ status: 200, contentType: 'application/json', body: '{"id":"new-channel"}' });
			return;
		}
		const body =
			path === '/api/v1/setup'
				? { needs_setup: false, requires_setup_token: false, name: 'Aegis Monitor', accent: '#a18aff' }
				: path === '/api/v1/me'
					? { id: 'owner', email: 'owner@example.test', role: 'owner' }
					: path === '/api/v1/monitors'
						? [monitorA, monitorB]
						: [];
		await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });
	});
	await page.goto('/notifications');
	await page.getByRole('button', { name: 'Add alert' }).first().click();
	const dialog = page.getByRole('dialog', { name: 'Create notification channel' });
	await expect(dialog.getByText('1 selected')).toBeVisible();
	await expect(dialog.getByRole('checkbox', { name: /API/ })).toBeChecked();
	await expect(dialog.getByRole('checkbox', { name: /Website/ })).not.toBeChecked();
	await dialog.getByRole('textbox', { name: 'Name', exact: true }).fill('API incidents');
	await dialog.getByLabel('Webhook URL').fill('https://discord.com/api/webhooks/123/secret');
	await dialog.getByRole('checkbox', { name: 'RECOVERED' }).uncheck();
	await dialog.getByRole('button', { name: 'Save channel' }).click();
	await expect.poll(() => saved?.monitor_ids).toEqual([monitorA.id]);
	await expect.poll(() => saved?.events).toEqual(['DOWN']);
});

test('opens an existing alert editor with literal template hints', async ({ page }) => {
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	const channel = {
		id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
		name: 'Discord alerts',
		provider: 'discord',
		enabled: true,
		events: ['DOWN'],
		monitor_ids: [],
		cooldown_seconds: 0,
		template: { title: '{{status}} · {{monitor}}', description: '{{reason}}', footer: '', fields: [] }
	};
	await page.route('**/api/v1/**', async (route) => {
		const path = new URL(route.request().url()).pathname;
		const body =
			path === '/api/v1/setup'
				? { needs_setup: false, requires_setup_token: false, name: 'Aegis Monitor', accent: '#a18aff' }
				: path === '/api/v1/me'
					? { id: 'owner', email: 'owner@example.test', role: 'owner' }
					: path === '/api/v1/channels'
						? [channel]
						: [];
		await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });
	});
	await page.goto('/notifications');
	await page.getByRole('button', { name: 'Edit', exact: true }).click();
	const dialog = page.getByRole('dialog', { name: 'Edit notification channel' });
	await expect(dialog).toBeVisible();
	await expect(dialog.getByLabel('Message content (outside embed)')).toHaveAttribute(
		'placeholder',
		'Optional message shown above the embed. Use {{monitor}} or {{status}}.'
	);
	await expect.poll(() => errors).toEqual([]);
});
