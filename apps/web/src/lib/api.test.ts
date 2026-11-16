import { describe, it, expect, vi, afterEach } from 'vitest';
import { api, duration, monitorStatus, percent, relative, type Monitor } from './api';
afterEach(() => vi.unstubAllGlobals());
describe('API contracts', () => {
	it('uses same-origin cookies and JSON for writes', async () => {
		const fetch = vi.fn().mockResolvedValue({ ok: true, json: async () => ({ id: 'test' }) });
		vi.stubGlobal('fetch', fetch);
		expect(await api('/monitors', 'POST', { name: 'test' })).toEqual({ id: 'test' });
		expect(fetch).toHaveBeenCalledWith(
			'/api/v1/monitors',
			expect.objectContaining({ credentials: 'same-origin', method: 'POST', body: '{"name":"test"}' })
		);
	});
	it('surfaces server errors without claiming success', async () => {
		vi.stubGlobal(
			'fetch',
			vi.fn().mockResolvedValue({ ok: false, json: async () => ({ error: 'Admin permission required' }) })
		);
		await expect(api('/monitors', 'POST', {})).rejects.toThrow('Admin permission required');
	});
	it('handles offline errors', async () => {
		vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('network')));
		await expect(api('/me')).rejects.toThrow('Unable to reach Aegis');
	});
	it('does not invent uptime for unknown periods', () => {
		expect(percent(null)).toBe('—');
		expect(percent(0)).toBe('0.00%');
		expect(percent(99.999)).toBe('100.00%');
	});
	it('formats real durations', () => {
		expect(duration('2026-01-01T00:00:00Z', '2026-01-01T00:01:12Z')).toBe('1m 12s');
		expect(relative(null)).toBe('Never connected');
	});
	it('shows a stopped PM2 process as offline while respecting the failure threshold', () => {
		const monitor = {
			kind: 'pm2',
			state: 'UP',
			last_error: 'PM2 process is offline (stopped)'
		} as Monitor;
		expect(monitorStatus(monitor)).toEqual({ state: 'PENDING', label: 'Confirming offline' });
		monitor.state = 'DOWN';
		expect(monitorStatus(monitor)).toEqual({ state: 'DOWN', label: 'Offline' });
		monitor.kind = 'http';
		expect(monitorStatus(monitor)).toEqual({ state: 'DOWN' });
	});
});
