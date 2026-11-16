<script lang="ts">
	import { ShieldCheck, ArrowRight, Eye, EyeOff, Activity, LockKeyhole } from '@lucide/svelte';
	import { api } from './api';
	let {
		setup = false,
		requiresSetupToken = false,
		name = 'Aegis Monitor',
		ondone
	}: { setup?: boolean; requiresSetupToken?: boolean; name?: string; ondone: () => void } = $props();
	let setupToken = $state('');
	let email = $state('');
	let password = $state('');
	let visible = $state(false);
	let busy = $state(false);
	let error = $state('');
	let recovery = $state(false);
	async function submit(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = '';
		try {
			if (recovery) {
				const r = await api<{ message: string }>('/auth/forgot', 'POST', { email });
				error = r.message;
			} else {
				await api(setup ? '/setup' : '/auth/login', 'POST', {
					email,
					password,
					setup_token: setup ? setupToken : undefined
				});
				ondone();
			}
		} catch (e) {
			error = (e as Error).message;
		} finally {
			busy = false;
		}
	}
</script>

<svelte:head><title>{setup ? 'Welcome' : 'Sign in'} · {name}</title></svelte:head>
<main class="auth-page">
	<div class="auth-atmosphere"></div>
	<a class="brand auth-brand" href="/"
		><span class="brand-mark"><ShieldCheck size={23} /></span>{name}<span class="edition">SELF-HOSTED</span
		></a
	>
	<div class="auth-layout">
		<section class="auth-story">
			<div class="eyebrow"><span class="live-dot"></span> YOUR INFRASTRUCTURE. IN FOCUS.</div>
			<h1>Quiet confidence.<br /><span>Every system.</span></h1>
			<p>The signal you need. The clarity you deserve.<br />A home for the health of everything you run.</p>
			<div class="auth-orbit">
				<div class="orbit-ring"></div>
				<div class="orbit-ring inner"></div>
				<div class="orbit-core"><ShieldCheck size={56} /></div>
				<div class="orbit-chip one"><Activity size={16} /> Endpoints</div>
				<div class="orbit-chip two"><span class="live-dot"></span> Infrastructure</div>
				<div class="orbit-chip three"><LockKeyhole size={16} /> Your data, your server</div>
			</div>
		</section>
		<section class="auth-card">
			<div class="eyebrow">
				{setup ? 'LET’S GET YOU STARTED' : recovery ? 'ACCOUNT RECOVERY' : 'WELCOME BACK'}
			</div>
			<h2>{setup ? 'Your command center awaits.' : recovery ? 'Recover access.' : 'Good to see you.'}</h2>
			<p class="muted">
				{setup
					? 'Create the owner account for your private instance. Public registration closes after this step.'
					: recovery
						? 'Recover your account using the instance owner’s CLI.'
						: 'Sign in to keep a pulse on your infrastructure.'}
			</p>
			<form onsubmit={submit} class="form-stack">
				{#if setup && requiresSetupToken}<label
						>Bootstrap token<input
							type="password"
							bind:value={setupToken}
							required
							autocomplete="off"
							minlength="32"
							placeholder="From your server environment"
						/></label
					>
					<p class="muted small-text">
						The one-time setup token prevents another visitor from claiming your production instance.
					</p>{/if}
				<label
					>Email address<input
						type="email"
						autocomplete="username"
						bind:value={email}
						required
						placeholder="you@example.com"
					/></label
				>{#if !recovery}<label
						>Password
						<div class="password-input">
							<input
								type={visible ? 'text' : 'password'}
								autocomplete={setup ? 'new-password' : 'current-password'}
								bind:value={password}
								required
								minlength={setup ? 12 : 1}
								maxlength="256"
								placeholder={setup ? 'At least 12 characters' : 'Enter your password'}
							/><button
								type="button"
								aria-label={visible ? 'Hide password' : 'Show password'}
								onclick={() => (visible = !visible)}
								>{#if visible}<EyeOff size={17} />{:else}<Eye size={17} />{/if}</button
							>
						</div></label
					>{/if}{#if error}<p class="error" role="alert">{error}</p>{/if}<button
					class="button primary full"
					disabled={busy}
					>{busy
						? 'Please wait…'
						: setup
							? 'Create owner account'
							: recovery
								? 'Recovery instructions'
								: 'Sign in'}<ArrowRight size={17} /></button
				>{#if !setup}<button
						type="button"
						class="text-button"
						onclick={() => {
							recovery = !recovery;
							error = '';
						}}>{recovery ? 'Back to sign in' : 'Forgot your password?'}</button
					>{/if}
			</form>
			<div class="auth-security"><LockKeyhole size={14} /> Private by design. Hosted by you.</div>
		</section>
	</div>
	<footer class="auth-footer">Built for the things you depend on.<span>{name} · v0.1</span></footer>
</main>
