<script lang="ts">
	import Seo from '#lib/components/Seo.svelte';
	import IntervalBar from '#lib/components/IntervalBar.svelte';
	import { record, signed } from '#lib/format.ts';

	let { data } = $props();
	let conference = $state('All');
	const conferences = $derived(['All', ...new Set(data.rows.map((r) => r.conference ?? 'Independent'))].sort((a, b) => (a === 'All' ? -1 : b === 'All' ? 1 : a.localeCompare(b))));
	const rows = $derived(
		conference === 'All' ? data.rows : data.rows.filter((r) => (r.conference ?? 'Independent') === conference)
	);
	const min = $derived(Math.floor(Math.min(...data.rows.map((r) => r.overall.value - 1.645 * r.overall.sd)) / 5) * 5);
	const max = $derived(Math.ceil(Math.max(...data.rows.map((r) => r.overall.value + 1.645 * r.overall.sd)) / 5) * 5);
	const move = (r: (typeof data.rows)[number]) => (r.previous_rank === null ? 0 : r.previous_rank - r.rank);
</script>

<Seo />

<div class="mb-5 flex flex-wrap items-end justify-between gap-4">
	<div>
		<h1 class="m-0 text-2xl font-bold tracking-tight">Who is actually best</h1>
		<p class="mt-1 max-w-[62ch] text-sm text-muted">
			Rating = expected margin against an average FBS team on a neutral field. The bar shows the 90% range; the
			rank range is where each team lands in 90% of plausible worlds. No preseason reputation, no conference
			bonus. <a href="/method" class="underline">How it works</a>.
		</p>
	</div>
	<label class="text-sm text-muted">
		Conference
		<select bind:value={conference} class="ml-1 rounded border border-border bg-surface px-2 py-1 text-text">
			{#each conferences as c (c)}<option value={c}>{c}</option>{/each}
		</select>
	</label>
</div>

<div class="overflow-x-auto rounded-lg border border-border bg-surface">
	<table class="num w-full text-sm">
		<thead>
			<tr class="border-b border-border text-left text-xs text-muted">
				<th class="px-3 py-2 font-medium">Rank</th>
				<th class="px-2 py-2 font-medium">Team</th>
				<th class="px-2 py-2 font-medium">Record</th>
				<th class="px-2 py-2 text-right font-medium">Rating</th>
				<th class="hidden w-[22%] px-2 py-2 font-medium md:table-cell"><span class="sr-only">90% range</span></th>
				<th class="hidden px-2 py-2 text-right font-medium sm:table-cell">Off</th>
				<th class="hidden px-2 py-2 text-right font-medium sm:table-cell">Def</th>
				<th class="hidden px-2 py-2 font-medium lg:table-cell">Last game</th>
				<th class="hidden px-3 py-2 text-right font-medium lg:table-cell" title="Average opponent rating">Sched</th>
			</tr>
		</thead>
		<tbody>
			{#each rows as r (r.slug)}
				<tr class="border-b border-border last:border-0 hover:bg-surface-2">
					<td class="px-3 py-2 whitespace-nowrap">
						<span class="font-semibold">{r.rank}</span>
						<span class="ml-1 text-xs text-faint" title="90% rank range">{r.rank_low}–{r.rank_high}</span>
						{#if move(r) !== 0}
							<span class="ml-1 text-xs {move(r) > 0 ? 'text-up' : 'text-down'}"
								>{move(r) > 0 ? '▲' : '▼'}{Math.abs(move(r))}</span
							>
						{/if}
					</td>
					<td class="px-2 py-2">
						<a href="/team/{r.slug}" class="font-medium no-underline hover:underline">{r.team}</a>
						<div class="text-xs text-faint">{r.conference ?? 'Independent'}</div>
					</td>
					<td class="px-2 py-2 whitespace-nowrap">
						{record(r.record.wins, r.record.losses)}
						<span class="text-xs text-faint">({record(r.record.conf_wins, r.record.conf_losses)})</span>
					</td>
					<td class="px-2 py-2 text-right whitespace-nowrap">
						<span class="font-semibold">{signed(r.overall.value)}</span>
						<span class="text-xs text-faint">±{(1.645 * r.overall.sd).toFixed(1)}</span>
					</td>
					<td class="hidden px-2 py-2 md:table-cell"><IntervalBar value={r.overall.value} sd={r.overall.sd} {min} {max} /></td>
					<td class="hidden px-2 py-2 text-right sm:table-cell">{signed(r.offense.value)}</td>
					<td class="hidden px-2 py-2 text-right sm:table-cell">{signed(r.defense.value)}</td>
					<td class="hidden px-2 py-2 text-xs whitespace-nowrap text-muted lg:table-cell">
						{#if r.last_game}
							{r.last_game.points_for > r.last_game.points_against ? 'W' : 'L'}
							{r.last_game.points_for}–{r.last_game.points_against}
							{r.last_game.site === 'away' ? '@' : 'vs'}
							{r.last_game.opponent}
						{/if}
					</td>
					<td class="hidden px-3 py-2 text-right lg:table-cell">{signed(r.schedule_strength)}</td>
				</tr>
			{/each}
		</tbody>
	</table>
</div>
