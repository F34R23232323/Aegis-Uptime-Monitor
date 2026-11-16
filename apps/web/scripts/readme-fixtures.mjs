// Synthetic data for documentation screenshots only. Never use production data here.
const now = Date.now();
const at = (minutesAgo) => new Date(now - minutesAgo * 60_000).toISOString();
const future = (minutesAhead) => new Date(now + minutesAhead * 60_000).toISOString();
const id = (number) => `00000000-0000-4000-8000-${String(number).padStart(12, '0')}`;

const specs = [
	['Public website', 'https://example.com', 'http', 'UP', 96, 'Web'],
	['API health', 'https://api.example.com/health', 'http', 'UP', 71, 'Services'],
	['PostgreSQL', 'db.example.com:5432', 'tcp', 'UP', 18, 'Infrastructure'],
	['Background worker', 'worker-01', 'pm2', 'DOWN', null, 'Services']
];
export const monitors = specs.map(([name, target, kind, state, latency, group], index) => ({
	id: id(index + 1),
	name,
	target,
	kind,
	state,
	enabled: true,
	interval_seconds: 30,
	timeout_seconds: 10,
	failure_threshold: 2,
	recovery_threshold: 2,
	group_name: group,
	description: '',
	tags: [],
	public: index < 3,
	server_id: kind === 'pm2' ? id(20) : null,
	config: kind === 'http' ? { method: 'GET', status: 200, retention_days: 90 } : {},
	last_latency: latency,
	last_error: state === 'DOWN' ? 'Process is offline' : null,
	last_checked_at: at(1)
}));

const makeChecks = (monitor) =>
	Array.from({ length: 48 }, (_, index) => {
		const down = monitor.state === 'DOWN' && index > 37;
		return {
			id: index + 1,
			ok: !down,
			state: down ? 'DOWN' : 'UP',
			latency_ms: monitor.last_latency || 0,
			error: down ? 'Process is offline' : null,
			observed: {},
			created_at: at(48 - index)
		};
	});
const uptime = (days, down = false) => ({
	days,
	uptime: down ? 98.8 : 100,
	covered_seconds: days * 86_400,
	up_seconds: days * 86_400 - (down ? 1_037 : 0),
	down_seconds: down ? 1_037 : 0
});
export const summaries = monitors.map((monitor) => ({
	id: monitor.id,
	checks: makeChecks(monitor),
	uptime: uptime(1, monitor.state === 'DOWN')
}));

export const incidents = [
	{
		id: id(101),
		title: 'Background worker offline',
		severity: 'major',
		phase: 'investigating',
		public_message: 'We are investigating delayed background jobs.',
		published: true,
		resource_ids: [id(301)],
		status_page_ids: [id(401)],
		updated_at: at(2),
		monitor_id: monitors[3].id,
		monitor_name: monitors[3].name,
		started_at: at(8),
		ended_at: null,
		cause: 'Process is offline',
		note: 'Checking worker logs.'
	}
];

export const servers = [
	{
		id: id(20),
		name: 'worker-01',
		last_seen: at(0),
		version: '0.1.0',
		inventory: {
			host: {
				cpu_percent: 28,
				memory_percent: 46,
				memory_total: 8_589_934_592,
				memory_used: 3_951_369_912,
				load: 0.7,
				uptime_seconds: 518_400,
				disks: [{ mount: '/', total: 107_374_182_400, available: 64_424_509_440 }],
				network: [{ name: 'eth0', received: 12_000_000, transmitted: 8_000_000 }]
			},
			docker: [
				{ id: 'web-container', name: 'web', state: 'running', healthy: true, health: 'healthy', restarts: 0 }
			],
			pm2: [{ id: 'worker-01', name: 'Background worker', state: 'stopped', healthy: false, restarts: 1 }]
		}
	}
];

export const resources = [
	{
		id: id(301),
		name: 'Worker jobs',
		description: 'Background processing and scheduled tasks.',
		group_name: 'Services',
		monitor_ids: [monitors[3].id],
		public: true,
		override_state: 'degraded',
		override_reason: 'Jobs are processing slowly while we investigate.',
		override_until: future(120),
		updated_at: at(5)
	},
	{
		id: id(302),
		name: 'Website and API',
		description: 'The public website and application API.',
		group_name: 'Core platform',
		monitor_ids: [monitors[0].id, monitors[1].id],
		public: true,
		override_state: null,
		override_reason: '',
		override_until: null,
		updated_at: at(10)
	},
	{
		id: id(303),
		name: 'Database',
		description: 'Primary data storage.',
		group_name: 'Core platform',
		monitor_ids: [monitors[2].id],
		public: true,
		override_state: null,
		override_reason: '',
		override_until: null,
		updated_at: at(10)
	}
];
export const tabs = [
	{ id: 'overview', label: 'Overview', kind: 'overview' },
	{ id: 'incidents', label: 'Incidents', kind: 'incidents' },
	{ id: 'maintenance', label: 'Maintenance', kind: 'maintenance' },
	{ id: 'metrics', label: 'Metrics', kind: 'metrics' },
	{ id: 'support', label: 'Support', kind: 'text', body: 'For help, contact your service operator.' }
];
export const page = {
	id: id(401),
	slug: 'demo',
	title: 'Aegis Demo Status',
	description: 'Live service health and planned maintenance.',
	accent: '#a18aff',
	theme: 'dark',
	layout: 'standard',
	logo_url: '',
	support_url: '',
	announcement: '',
	tabs,
	resource_ids: resources.map((resource) => resource.id),
	monitor_ids: [],
	metrics_settings: {
		show_uptime: true,
		show_latency: true,
		show_on_overview: true,
		uptime_days: [1, 7, 30, 90],
		latency_samples: 120
	}
};

export const maintenance = [
	{
		id: id(501),
		reason: 'Database migration rehearsal',
		title: 'Scheduled platform maintenance',
		public_message: 'Brief interruptions are possible during this window.',
		starts_at: future(1_440),
		ends_at: future(1_500),
		monitor_ids: [monitors[0].id],
		resource_ids: [resources[1].id],
		status_page_ids: [page.id]
	}
];
export const channels = [
	{
		id: id(601),
		name: 'Operations Discord',
		provider: 'discord',
		enabled: true,
		events: ['DOWN', 'RECOVERED'],
		monitor_ids: [monitors[0].id, monitors[1].id],
		cooldown_seconds: 300,
		template: {
			content: '',
			mention_users: false,
			mention_roles: false,
			mention_everyone: false,
			title: '{{status}} · {{monitor}}',
			description: '{{reason}}',
			footer: '{{brand}}',
			username: 'Aegis Monitor',
			avatar_url: '',
			thumbnail_url: '',
			down_color: '#ed4245',
			recovered_color: '#57f287',
			degraded_color: '#fee75c',
			show_timestamp: true,
			fields: [{ name: 'Duration', value: '{{duration}}', inline: true }]
		}
	}
];
export const deliveries = [
	{
		id: id(701),
		channel_name: channels[0].name,
		event: 'DOWN',
		state: 'sent',
		attempts: 1,
		last_error: null,
		created_at: at(7)
	}
];
export const users = [
	{ id: id(201), email: 'demo@example.com', role: 'owner' },
	{ id: id(202), email: 'viewer@example.com', role: 'viewer' }
];
export const sessions = [{ id: id(801), created_at: at(60), expires_at: future(10_000), current: true }];
export const audit = [
	{
		id: 1,
		action: 'resource.override',
		target_id: resources[0].id,
		actor: users[0].email,
		created_at: at(5),
		details: {}
	}
];
export const incidentUpdates = [
	{ id: id(901), phase: 'investigating', message: 'We are reviewing the worker queue.', created_at: at(3) }
];

const publicUptime = [uptime(1), uptime(7), uptime(30), uptime(90)];
const latency = Array.from({ length: 36 }, (_, index) => ({
	at: at(36 - index),
	latency_ms: 70 + (index % 7) * 4
}));
export const publicPage = {
	...page,
	brand: 'Aegis Monitor',
	generated_at: at(0),
	overall_state: 'degraded',
	metrics_summary: { uptime: [1, 7, 30, 90].map((days) => uptime(days, true)), latency },
	resources: resources.map((resource) => ({
		id: resource.id,
		name: resource.name,
		description: resource.description,
		group_name: resource.group_name,
		state: resource.override_state || 'operational',
		status_source: resource.override_state ? 'manual' : 'automatic',
		override_reason: resource.override_reason,
		override_until: resource.override_until,
		uptime: resource.override_state ? [1, 7, 30, 90].map((days) => uptime(days, true)) : publicUptime,
		latency
	})),
	monitors: monitors
		.filter((monitor) => monitor.public)
		.map((monitor) => ({
			id: monitor.id,
			name: monitor.name,
			state: monitor.state,
			latency,
			history: makeChecks(monitor).map((check) => ({ state: check.state, at: check.created_at })),
			incidents: [],
			uptime_24h: uptime(1),
			uptime_7d: uptime(7),
			uptime_30d: uptime(30),
			uptime_90d: uptime(90)
		})),
	incidents: [
		{
			id: incidents[0].id,
			title: incidents[0].title,
			severity: 'major',
			phase: 'investigating',
			message: incidents[0].public_message,
			resource_ids: [resources[0].id],
			started_at: incidents[0].started_at,
			ended_at: null,
			updated_at: incidents[0].updated_at,
			updates: incidentUpdates
		}
	],
	maintenance: [
		{
			id: maintenance[0].id,
			title: maintenance[0].title,
			message: maintenance[0].public_message,
			starts_at: maintenance[0].starts_at,
			ends_at: maintenance[0].ends_at
		}
	]
};

export const providerSummary = {
	page: { updated_at: at(3) },
	status: { description: 'All systems operational', indicator: 'none' },
	components: [
		{
			id: 'group-1',
			name: 'Cloudflare services',
			status: 'operational',
			group: true,
			group_id: null,
			description: null
		},
		{
			id: 'component-1',
			name: 'CDN',
			status: 'operational',
			group: false,
			group_id: 'group-1',
			description: 'Content delivery network'
		},
		{
			id: 'component-2',
			name: 'DNS',
			status: 'operational',
			group: false,
			group_id: 'group-1',
			description: 'Authoritative DNS'
		}
	],
	incidents: []
};

export function responseFor(path, method) {
	if (path === '/setup')
		return { needs_setup: false, requires_setup_token: false, name: 'Aegis Monitor', accent: '#a18aff' };
	if (path === '/me') return users[0];
	if (path === '/monitors') return monitors;
	if (path === '/monitors/counts')
		return { total: monitors.length, states: { UP: 3, DOWN: 1 }, open_incidents: 1 };
	if (path === '/monitors/summaries') return summaries;
	if (path === `/monitors/${monitors[0].id}/checks`) return makeChecks(monitors[0]);
	if (path === `/monitors/${monitors[0].id}/uptime`) return uptime(1);
	if (path === '/incidents') return incidents;
	if (path === `/incidents/${incidents[0].id}/updates`) return incidentUpdates;
	if (path === '/servers') return servers;
	if (path === '/status-pages') return [page];
	if (path === '/resources') return resources;
	if (path === '/maintenance') return maintenance;
	if (path === '/channels') return channels;
	if (path === '/deliveries') return deliveries;
	if (path === '/auth/sessions') return sessions;
	if (path === '/audit') return audit;
	if (path === '/users') return users;
	if (path === '/system')
		return { version: '0.1.0', app_url: 'https://status.example.com', private_targets: false };
	if (path === '/channels/preview' && method === 'POST')
		return {
			username: 'Aegis Monitor',
			content: '',
			embeds: [
				{
					title: 'DOWN · Public website',
					description: 'The service is not responding.',
					color: 15548997,
					footer: { text: 'Aegis Monitor' },
					fields: [{ name: 'Duration', value: '8m', inline: true }]
				}
			]
		};
	return [];
}
