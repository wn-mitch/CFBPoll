<script lang="ts">
	import Seo from '#lib/components/Seo.svelte';
	import { pct } from '#lib/format.ts';

	let { data } = $props();
	let showAll = $state(false);
	const rows = $derived(showAll ? data.rows : data.rows.filter((r) => r.playoff >= 0.005 || r.conf_title >= 0.05));
	// Shading for probability cells: one hue, light to dark.
	const shade = (p: number) => `color-mix(in oklab, var(--series-1) ${Math.round(p * 55)}%, transparent)`;
</script>

<Seo title="Playoff odds" description="Conference title, playoff, and national title odds from {data.meta.draws.toLocaleString()} simulated seasons." />

<h1 class="mt-0 mb-1 text-2xl font-bold tracking-tight">Playoff odds</h1>
<p class="mt-0 mb-4 max-w-[70ch] text-sm text-muted">
	From {data.meta.draws.toLocaleString()} simulations of the rest of the season. Each draws every team's true strength
	from the model's uncertainty, plays the remaining games, conference title games and the 12-team bracket. Selection
	assumes the committee ranks teams the way this model would at season's end, with five conference-champion auto-bids
	and Notre Dame's top-12 rule. Tiebreakers are simplified.
</p>

<div class="overflow-x-auto rounded-lg border border-border bg-surface">
	<table class="num w-full text-sm">
		<thead>
			<tr class="border-b border-border text-left text-xs text-muted">
				<th class="px-3 py-2 font-medium">Team</th>
				<th class="px-2 py-2 text-right font-medium">Proj. record</th>
				<th class="hidden px-2 py-2 text-right font-medium sm:table-cell">Title game</th>
				<th class="px-2 py-2 text-right font-medium">Conf. title</th>
				<th class="px-2 py-2 text-right font-medium">Playoff</th>
				<th class="hidden px-2 py-2 text-right font-medium sm:table-cell">Bye</th>
				<th class="hidden px-2 py-2 text-right font-medium md:table-cell">Semis</th>
				<th class="px-3 py-2 text-right font-medium">Title</th>
			</tr>
		</thead>
		<tbody>
			{#each rows as r (r.slug)}
				<tr class="border-b border-border last:border-0">
					<td class="px-3 py-1.5">
						<span class="text-xs text-faint">{r.rank}</span>
						<a href="/team/{r.slug}" class="no-underline hover:underline">{r.team}</a>
						<span class="hidden text-xs text-faint sm:inline">{r.conference ?? 'Independent'}</span>
					</td>
					<td class="px-2 py-1.5 text-right whitespace-nowrap">{r.expected_wins.toFixed(1)}–{r.expected_losses.toFixed(1)}</td>
					<td class="hidden px-2 py-1.5 text-right sm:table-cell" style="background: {shade(r.conf_title_game)}">{pct(r.conf_title_game)}</td>
					<td class="px-2 py-1.5 text-right" style="background: {shade(r.conf_title)}">{pct(r.conf_title)}</td>
					<td class="px-2 py-1.5 text-right font-semibold" style="background: {shade(r.playoff)}">{pct(r.playoff)}</td>
					<td class="hidden px-2 py-1.5 text-right sm:table-cell" style="background: {shade(r.bye)}">{pct(r.bye)}</td>
					<td class="hidden px-2 py-1.5 text-right md:table-cell" style="background: {shade(r.semifinal)}">{pct(r.semifinal)}</td>
					<td class="px-3 py-1.5 text-right" style="background: {shade(r.title)}">{pct(r.title)}</td>
				</tr>
			{/each}
		</tbody>
	</table>
</div>
<button class="mt-3 rounded border border-border bg-surface px-3 py-1 text-sm hover:bg-surface-2" onclick={() => (showAll = !showAll)}>
	{showAll ? 'Show contenders only' : `Show all ${data.rows.length} teams`}
</button>
