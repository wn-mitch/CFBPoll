<script lang="ts">
	import './layout.css';
	import { page } from '$app/state';

	let { data, children } = $props();
	const links = [
		{ href: '/', label: 'Rankings' },
		{ href: '/games', label: 'Games' },
		{ href: '/odds', label: 'Playoff odds' },
		{ href: '/conferences', label: 'Conferences' },
		{ href: '/polls', label: 'vs. the polls' },
		{ href: '/method', label: 'Method' }
	];
	const active = (href: string) =>
		href === '/' ? page.url.pathname === '/' || page.url.pathname.startsWith('/team') : page.url.pathname.startsWith(href);
</script>

<div class="min-h-screen">
	<header class="border-b border-border bg-surface">
		<div class="mx-auto flex max-w-6xl flex-wrap items-baseline gap-x-6 gap-y-2 px-4 py-3">
			<a href="/" class="text-lg font-bold tracking-tight no-underline">CFB Poll</a>
			<span class="text-sm text-muted">{data.meta.year} · through {data.meta.as_of}</span>
			<nav class="-mx-1 flex w-full gap-1 overflow-x-auto text-sm sm:ml-auto sm:w-auto" aria-label="Main">
				{#each links as l (l.href)}
					<a
						href={l.href}
						class="whitespace-nowrap rounded px-2 py-1 no-underline hover:bg-surface-2 {active(l.href)
							? 'bg-surface-2 font-semibold text-text'
							: 'text-muted'}"
						aria-current={active(l.href) ? 'page' : undefined}>{l.label}</a
					>
				{/each}
			</nav>
		</div>
	</header>
	<main class="mx-auto max-w-6xl px-4 py-6">
		{@render children()}
	</main>
	<footer class="mx-auto max-w-6xl px-4 pb-10 pt-4 text-xs text-faint">
		Data: <a href="https://collegefootballdata.com" class="underline">CollegeFootballData.com</a>. Updated
		{data.meta.generated}. <a href="https://github.com/wn-mitch/CFBPoll" class="underline">Source</a>.
	</footer>
</div>
