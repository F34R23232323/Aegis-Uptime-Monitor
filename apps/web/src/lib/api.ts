export interface Monitor {
	has_credentials?: boolean;
	id: string;
	name: string;
	kind: string;
	target: string;
	state: string;
	enabled: boolean;
	interval_seconds: number;
	timeout_seconds: number;
	failure_threshold: number;
	failures?: number;
	recovery_threshold: number;
	group_name: string;
	description: string;
	tags: string[];
	public: boolean;
	server_id: string | null;
	depends_on: string | null;
	config: Record<string, unknown>;
	last_latency: number | null;
	last_error: string | null;
	last_checked_at: string | null;
}
export function monitorStatus(monitor: Monitor): { state: string; label?: string } {
	if (monitor.kind === 'pm2') {
		if (monitor.state === 'DOWN') return { state: 'DOWN', label: 'Offline' };
		if (monitor.state === 'UP' && monitor.last_error?.startsWith('PM2 process is offline')) {
			return { state: 'PENDING', label: 'Confirming offline' };
		}
	}
	return { state: monitor.state };
}
export interface Check {
	id: number;
	ok: boolean;
	state: string;
	latency_ms: number;
	error: string | null;
	created_at: string;
	observed: Record<string, unknown>;
}
export interface Incident {
	title: string;
	severity: string;
	phase: string;
	public_message: string;
	published: boolean;
	resource_ids: string[];
	status_page_ids: string[];
	updated_at: string;
	id: string;
	monitor_id: string | null;
	monitor_name: string | null;
	started_at: string;
	ended_at: string | null;
	cause: string;
	note: string;
}
export interface Service {
	id: string;
	name: string;
	state?: string;
	status?: string;
	healthy: boolean;
	restarts?: number;
	health?: string;
	labels?: Record<string, string>;
}
export interface Server {
	id: string;
	name: string;
	last_seen: string | null;
	version: string | null;
	inventory: {
		host?: {
			cpu_percent: number;
			memory_percent: number;
			memory_total: number;
			memory_used: number;
			load: number;
			uptime_seconds: number;
			disks: { mount: string; total: number; available: number }[];
			network: { name: string; received: number; transmitted: number }[];
		};
		docker?: Service[];
		pm2?: Service[];
		docker_error?: string;
		pm2_errors?: string[];
	};
}
export interface Channel {
	template: EmbedTemplate;
	events: string[];
	monitor_ids: string[];
	cooldown_seconds: number;
	quiet_start_minute: number | null;
	quiet_end_minute: number | null;
	delay_seconds: number;
	id: string;
	name: string;
	provider: string;
	enabled: boolean;
}
export interface HostMetricSample {
	bucket: string;
	cpu_percent: number | null;
	memory_percent: number | null;
	disk_percent: number | null;
}
export interface Delivery {
	id: string;
	channel_name: string;
	event: string;
	state: string;
	attempts: number;
	last_error: string | null;
	created_at: string;
}
export interface Page {
	layout: 'standard' | 'provider';
	metrics_settings: StatusMetricsSettings;
	resource_ids: string[];
	tabs: StatusTab[];
	theme: 'dark' | 'light' | 'auto';
	logo_url: string;
	support_url: string;
	support_label: string;
	footer_text: string;
	announcement: string;
	id: string;
	slug: string;
	title: string;
	description: string;
	accent: string;
	monitor_ids: string[];
}
export interface StatusMetricsSettings {
	show_uptime: boolean;
	show_latency: boolean;
	show_on_overview: boolean;
	uptime_days: number[];
	latency_samples: 60 | 120 | 240;
}
export const defaultMetricsSettings: StatusMetricsSettings = {
	show_uptime: true,
	show_latency: true,
	show_on_overview: true,
	uptime_days: [1, 7, 30, 90],
	latency_samples: 120
};
export interface Maintenance {
	title: string;
	public_message: string;
	status_page_ids: string[];
	resource_ids: string[];
	id: string;
	reason: string;
	starts_at: string;
	ends_at: string;
	monitor_ids: string[];
}
export interface Uptime {
	days: number;
	uptime: number | null;
	covered_seconds: number;
	up_seconds: number;
	down_seconds: number;
}
export interface User {
	id: string;
	email: string;
	role: 'owner' | 'admin' | 'viewer';
}
export async function api<T>(path: string, method = 'GET', body?: unknown): Promise<T> {
	let r: Response;
	try {
		r = await fetch('/api/v1' + path, {
			method,
			headers: body ? { 'content-type': 'application/json' } : {},
			body: body ? JSON.stringify(body) : undefined,
			credentials: 'same-origin'
		});
	} catch {
		throw new Error('Unable to reach Aegis. Check your connection and try again.');
	}
	if (!r.ok) {
		const error = await r.json().catch(() => ({ error: `Request failed (${r.status})` }));
		throw new Error(error.error || 'Request failed');
	}
	return r.json();
}
export const time = (date: string | null) =>
	date ? new Date(date).toLocaleString() : 'Awaiting first check';
export const relative = (date: string | null) => {
	if (!date) return 'Never connected';
	const seconds = Math.max(0, Math.round((Date.now() - new Date(date).getTime()) / 1000));
	return seconds < 60
		? `${seconds}s ago`
		: seconds < 3600
			? `${Math.floor(seconds / 60)}m ago`
			: `${Math.floor(seconds / 3600)}h ago`;
};
export const duration = (start: string, end?: string | null) => {
	const seconds = Math.max(
		0,
		Math.round(((end ? new Date(end).getTime() : Date.now()) - new Date(start).getTime()) / 1000)
	);
	return seconds < 60
		? `${seconds}s`
		: seconds < 3600
			? `${Math.floor(seconds / 60)}m ${seconds % 60}s`
			: `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`;
};
export const percent = (n: number | null | undefined) => (n == null ? '—' : `${n.toFixed(2)}%`);
export type ResourceState =
	'operational' | 'degraded' | 'partial_outage' | 'major_outage' | 'maintenance' | 'unknown';
export interface StatusResource {
	id: string;
	name: string;
	description: string;
	group_name: string;
	monitor_ids: string[];
	public: boolean;
	override_state: ResourceState | null;
	override_reason: string;
	override_until: string | null;
	updated_at: string;
}
export interface StatusTab {
	id: string;
	label: string;
	kind: 'overview' | 'incidents' | 'maintenance' | 'metrics' | 'text';
	body?: string;
}
export interface IncidentUpdate {
	id: string;
	phase: string;
	message: string;
	created_at: string;
}
export interface EmbedField {
	name: string;
	value: string;
	inline: boolean;
}
export interface EmbedTemplate {
	content: string;
	mention_users: boolean;
	mention_roles: boolean;
	mention_everyone: boolean;
	title: string;
	description: string;
	footer: string;
	username: string;
	avatar_url: string;
	thumbnail_url: string;
	down_color: string;
	recovered_color: string;
	degraded_color: string;
	show_timestamp: boolean;
	fields: EmbedField[];
}
export interface DiscordPayload {
	content?: string;
	username?: string;
	avatar_url?: string;
	embeds: {
		title: string;
		description: string;
		color: number;
		timestamp?: string;
		footer: { text: string };
		fields?: EmbedField[];
		thumbnail?: { url: string };
	}[];
}
export interface SessionInfo {
	id: string;
	created_at: string;
	expires_at: string;
	current: boolean;
}
export interface AuditEntry {
	id: number;
	action: string;
	target_id: string;
	actor: string | null;
	created_at: string;
	details: Record<string, unknown>;
}
export const resourceStates: { value: ResourceState; label: string }[] = [
	{ value: 'operational', label: 'Operational' },
	{ value: 'degraded', label: 'Degraded performance' },
	{ value: 'partial_outage', label: 'Partial outage' },
	{ value: 'major_outage', label: 'Major outage' },
	{ value: 'maintenance', label: 'Under maintenance' },
	{ value: 'unknown', label: 'Unknown' }
];
export const defaultTabs: StatusTab[] = [
	{ id: 'overview', label: 'Overview', kind: 'overview' },
	{ id: 'incidents', label: 'Incidents', kind: 'incidents' },
	{ id: 'maintenance', label: 'Maintenance', kind: 'maintenance' },
	{ id: 'metrics', label: 'Metrics', kind: 'metrics' }
];
export const defaultEmbed: EmbedTemplate = {
	content: '',
	mention_users: false,
	mention_roles: false,
	mention_everyone: false,
	title: '{{status}} · {{monitor}}',
	description: '{{reason}}',
	footer: '{{brand}}',
	username: '',
	avatar_url: '',
	thumbnail_url: '',
	down_color: '#ed4245',
	recovered_color: '#57f287',
	degraded_color: '#fee75c',
	show_timestamp: true,
	fields: [
		{ name: 'Duration', value: '{{duration}}', inline: true },
		{ name: 'Dashboard', value: '[Open monitor]({{dashboard_url}})', inline: true }
	]
};
