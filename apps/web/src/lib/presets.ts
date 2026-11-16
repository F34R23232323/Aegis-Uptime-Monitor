export const presets = [
	{
		id: 'cloudflare',
		name: 'Cloudflare',
		category: 'Network',
		origin: 'https://www.cloudflarestatus.com',
		color: '#f5a623'
	},
	{
		id: 'proton',
		name: 'Proton VPN & services',
		category: 'Privacy',
		origin: 'https://status.proton.me',
		color: '#8b70df'
	},
	{
		id: 'github',
		name: 'GitHub',
		category: 'Developer',
		origin: 'https://www.githubstatus.com',
		color: '#a4a9b8'
	},
	{
		id: 'vercel',
		name: 'Vercel',
		category: 'Developer',
		origin: 'https://www.vercel-status.com',
		color: '#e4e5e9'
	},
	{
		id: 'discord',
		name: 'Discord',
		category: 'Communications',
		origin: 'https://discordstatus.com',
		color: '#8495ff'
	},
	{
		id: 'atlassian',
		name: 'Atlassian Developer',
		category: 'Productivity',
		origin: 'https://developer.status.atlassian.com',
		color: '#5b9bff'
	},
	{
		id: 'bitbucket',
		name: 'Bitbucket',
		category: 'Developer',
		origin: 'https://bitbucket.status.atlassian.com',
		color: '#5b9bff'
	},
	{
		id: 'trello',
		name: 'Trello',
		category: 'Productivity',
		origin: 'https://trello.status.atlassian.com',
		color: '#5b9bff'
	},
	{
		id: 'digitalocean',
		name: 'DigitalOcean',
		category: 'Cloud',
		origin: 'https://status.digitalocean.com',
		color: '#42a4ff'
	},
	{
		id: 'twilio',
		name: 'Twilio',
		category: 'Communications',
		origin: 'https://status.twilio.com',
		color: '#f46d74'
	},
	{
		id: '1password',
		name: '1Password',
		category: 'Security',
		origin: 'https://status.1password.com',
		color: '#69b8ff'
	},
	{
		id: 'google-cloud',
		name: 'Google Cloud',
		category: 'Cloud',
		origin: 'https://status.cloud.google.com',
		color: '#8eb6ff'
	}
] as const;

export type Preset = (typeof presets)[number];
export interface PresetComponent {
	id: string;
	name: string;
	status: string;
	group: boolean;
	group_id: string | null;
	description: string | null;
}
export interface PresetSummary {
	page: { updated_at?: string };
	status: { description: string; indicator: string };
	components: PresetComponent[];
	incidents: { id: string; name: string; status: string; updated_at: string }[];
}
