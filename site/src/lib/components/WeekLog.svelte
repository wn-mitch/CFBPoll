<script lang="ts">
	import type { WeekPoint } from '#lib/types.ts';
	import { signed } from '#lib/format.ts';
	import Logo from '#lib/components/Logo.svelte';

	// Most recent week first: the game(s) played, the rating change they
	// caused, and the rank move.
	let { points }: { points: WeekPoint[] } = $props();

	const rows = $derived(
		points
			.map((p, i) => ({ p, prev: i > 0 ? points[i - 1].rank : null }))
			.slice(1)
			.reverse()
	);
	const at = (site: string) => (site === 'away' ? '@' : site === 'neutral' ? 'vs' : '');
</script>

<ol class="num m-0 list-none p-0 text-sm">
	{#each rows as { p, prev } (p.label)}
		{@const moved = prev === null ? 0 : prev - p.rank}
		<li class="grid grid-cols-[3.25rem_1fr_auto_4.5rem] items-baseline gap-x-3 border-b border-border py-1.5 last:border-0">
			<span class="text-xs text-faint">{p.label.replace('Week ', 'Wk ')}</span>
			<span class="min-w-0">
				{#each p.games as g, gi (gi)}
					<span class="flex min-w-0 items-center gap-1.5">
						<span class="w-3 font-semibold {g.points_for > g.points_against ? 'text-up' : 'text-down'}"
							>{g.points_for > g.points_against ? 'W' : 'L'}</span
						>
						<span class="whitespace-nowrap">{g.points_for}–{g.points_against}</span>
						<span class="text-faint">{at(g.site)}</span>
						<Logo id={g.opponent_logo_id} size={16} />
						<span class="truncate">{g.opponent}</span>
					</span>
				{:else}
					<span class="text-faint">No game</span>
				{/each}
			</span>
			<span
				class="text-right text-xs {p.change === null || Math.abs(p.change) < 0.05
					? 'text-faint'
					: p.change > 0
						? 'text-up'
						: 'text-down'}"
				title="Rating change">{p.change === null ? '' : signed(p.change)}</span
			>
			<span class="text-right whitespace-nowrap">
				<span class="font-medium">#{p.rank}</span>
				{#if moved !== 0}
					<span class="text-xs {moved > 0 ? 'text-up' : 'text-down'}"
						><span aria-hidden="true">{moved > 0 ? '▲' : '▼'}</span><span class="sr-only">{moved > 0 ? 'up' : 'down'}</span
						>{Math.abs(moved)}</span
					>
				{:else}
					<span class="text-xs text-faint" aria-hidden="true">·</span>
				{/if}
			</span>
		</li>
	{/each}
</ol>
