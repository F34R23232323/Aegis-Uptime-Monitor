import type { StatusTab, Uptime, ResourceState, IncidentUpdate } from './api';
export interface PublicMonitor {
	latency: { at: string; latency_ms: number }[];
	id: string;
	name: string;
	state: string;
	history: { state: string; at: string }[];
	incidents: { started_at: string; ended_at: string | null }[];
	uptime_24h: Uptime;
	uptime_7d: Uptime;
	uptime_30d: Uptime;
	uptime_90d: Uptime;
}
export interface PublicResource {
	latency: { at: string; latency_ms: number }[];
	id: string;
	name: string;
	description: string;
	group_name: string;
	state: ResourceState;
	status_source: 'manual' | 'automatic';
	override_reason: string;
	override_until: string | null;
	uptime: Uptime[];
}
export interface PublicIncident {
	id: string;
	title: string;
	severity: string;
	phase: string;
	message: string;
	resource_ids: string[];
	started_at: string;
	ended_at: string | null;
	updated_at: string;
	updates: IncidentUpdate[];
}
export interface PublicMaintenance {
	id: string;
	title: string;
	message: string;
	starts_at: string;
	ends_at: string;
}
export interface PublicPage {
	layout: 'standard' | 'provider';
	metrics_settings: import('./api').StatusMetricsSettings;
	metrics_summary: { uptime: Uptime[]; latency: { at: string; latency_ms: number }[] };
	title: string;
	slug: string;
	description: string;
	accent: string;
	brand: string;
	theme: 'dark' | 'light' | 'auto';
	logo_url: string;
	support_url: string;
	support_label: string;
	footer_text: string;
	announcement: string;
	tabs: StatusTab[];
	resources: PublicResource[];
	monitors: PublicMonitor[];
	incidents: PublicIncident[];
	maintenance: PublicMaintenance[];
	overall_state: ResourceState;
	generated_at: string;
}
export const statusLabels: Record<ResourceState, string> = {
	operational: 'Operational',
	degraded: 'Degraded performance',
	partial_outage: 'Partial outage',
	major_outage: 'Major outage',
	maintenance: 'Under maintenance',
	unknown: 'Unknown'
};
