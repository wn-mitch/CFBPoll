<script lang="ts">
	import Seo from '#lib/components/Seo.svelte';
	import { pct, shortDate, spread } from '#lib/format.ts';
	import type { GamePrediction } from '#lib/types.ts';

	let { data } = $props();
	const g = $derived(data.games);
	const fav = (p: GamePrediction) => (p.home_win >= 0.5 ? p.home : p.away);
	const favP = (p: GamePrediction) => Math.max(p.home_win, 1 - p.home_win);
	const called = (p: GamePrediction) => p.result !== null && (p.result[0] > p.result[1]) === p.home_win >= 0.5;
	const recentHits = $derived(g.recent.filter(called).length);
</script>

<Seo title="Games" description="Win probabilities and projected scores for this week's college football games." />

{#snippet table(list: GamePrediction[], showResult: boolean)}
	<div class="overflow-x-auto rounded-lg border border-border bg-surface">
		<table class="num w-full text-sm">
			<thead>
				<tr class="border-b border-border text-left text-xs text-muted">
					<th class="px-3 py-2 font-medium">Matchup</th>
					<th class="px-2 py-2 font-medium">Favorite</th>
					<th class="hidden px-2 py-2 text-right font-medium sm:table-cell">Model score</th>
					<th class="hidden px-2 py-2 font-medium md:table-cell">Model line</th>
					<th class="hidden px-2 py-2 font-medium md:table-cell">Market</th>
					{#if showResult}<th class="px-3 py-2 font-medium">Final</th>{/if}
				</tr>
			</thead>
			<tbody>
				{#each list as p (p.id)}
					<tr class="border-b border-border last:border-0">
						<td class="px-3 py-2">
							<div>
								{#if p.away_rank && p.away_rank <= 25}<span class="text-xs text-faint">{p.away_rank}</span>{/if}
								{#if p.away_slug}<a class="no-underline hover:underline" href="/team/{p.away_slug}">{p.away}</a>{:else}{p.away}{/if}
								<span class="text-faint">{p.neutral ? 'vs' : '@'}</span>
								{#if p.home_rank && p.home_rank <= 25}<span class="text-xs text-faint">{p.home_rank}</span>{/if}
								{#if p.home_slug}<a class="no-underline hover:underline" href="/team/{p.home_slug}">{p.home}</a>{:else}{p.home}{/if}
							</div>
							<div class="text-xs text-faint">{shortDate(p.date)}</div>
						</td>
						<td class="px-2 py-2 whitespace-nowrap">{fav(p)} <span class="font-semibold">{pct(favP(p))}</span></td>
						<td class="hidden px-2 py-2 text-right whitespace-nowrap text-muted sm:table-cell"
							>{Math.round(p.away_points)}–{Math.round(p.home_points)}</td
						>
						<td class="hidden px-2 py-2 whitespace-nowrap md:table-cell">{spread(p.home_points - p.away_points, p.home, p.away)}</td>
						<td class="hidden px-2 py-2 whitespace-nowrap text-muted md:table-cell"
							>{p.market_margin === null ? '' : spread(p.market_margin, p.home, p.away)}</td
						>
						{#if showResult}
							<td class="px-3 py-2 whitespace-nowrap">
								{#if p.result}
									{p.result[1]}–{p.result[0]}
									<span class="ml-1 text-xs {called(p) ? 'text-up' : 'text-down'}">{called(p) ? 'called' : 'missed'}</span>
								{:else}<span class="text-faint">—</span>{/if}
							</td>
						{/if}
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
{/snippet}

<h1 class="mt-0 mb-1 text-2xl font-bold tracking-tight">{g.upcoming_label ?? 'Games'}</h1>
<p class="mt-0 mb-4 text-sm text-muted">
	Scores are listed away–home. The model line is its expected margin; the market is the closing spread.
</p>
{#if g.upcoming.length}
	{@render table(g.upcoming, true)}
{:else}
	<p class="text-muted">No games left this season.</p>
{/if}

{#if g.recent.length}
	<h2 class="mt-10 mb-1 text-lg font-semibold">{g.recent_label} results</h2>
	<p class="mt-0 mb-4 text-sm text-muted">
		Pregame favorite won {recentHits} of {g.recent.filter((p) => p.result).length}.
	</p>
	{@render table(g.recent, true)}
{/if}
