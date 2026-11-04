<script lang="ts">
	import Seo from '#lib/components/Seo.svelte';
	import Trajectory from '#lib/components/Trajectory.svelte';
	import Logo from '#lib/components/Logo.svelte';
	import { pct, record, shortDate, signed } from '#lib/format.ts';

	let { data } = $props();
	const t = $derived(data.team);
	const r = $derived(t.row);
	const z = 1.645;
</script>

<Seo
	title={r.team}
	description="{r.team}: #{r.rank} (range {r.rank_low}–{r.rank_high}), rating {signed(r.overall.value)} points vs. an average FBS team."
/>

<a href="/" class="text-sm text-muted no-underline hover:underline">← Rankings</a>
<div class="mt-2 mb-6 flex flex-wrap items-end gap-x-8 gap-y-3">
	<div class="flex items-center gap-4">
		<Logo id={r.logo_id} size={64} />
		<div>
		<div class="text-sm text-muted">
			{r.conference ?? 'Independent'} · {record(r.record.wins, r.record.losses)}
			({record(r.record.conf_wins, r.record.conf_losses)} conf)
		</div>
		<h1 class="m-0 text-3xl font-bold tracking-tight">{r.team}</h1>
		</div>
	</div>
	<dl class="num m-0 flex flex-wrap gap-x-8 gap-y-2">
		<div>
			<dt class="text-xs text-muted">Rank</dt>
			<dd class="m-0 text-2xl font-semibold">
				#{r.rank} <span class="text-sm font-normal text-faint">{r.rank_low}–{r.rank_high}</span>
			</dd>
		</div>
		<div>
			<dt class="text-xs text-muted">Rating</dt>
			<dd class="m-0 text-2xl font-semibold">
				{signed(r.overall.value)} <span class="text-sm font-normal text-faint">±{(z * r.overall.sd).toFixed(1)}</span>
			</dd>
		</div>
		<div>
			<dt class="text-xs text-muted">Offense</dt>
			<dd class="m-0 text-2xl font-semibold">{signed(r.offense.value)}</dd>
		</div>
		<div>
			<dt class="text-xs text-muted">Defense</dt>
			<dd class="m-0 text-2xl font-semibold">{signed(r.defense.value)}</dd>
		</div>
	</dl>
</div>

<div class="grid gap-6 lg:grid-cols-[1.4fr_1fr]">
	<section class="rounded-lg border border-border bg-surface p-4">
		<h2 class="mt-0 mb-3 text-base font-semibold">Rating by week</h2>
		<Trajectory points={t.trajectory} />
	</section>
	{#if t.odds}
		{@const o = t.odds}
		<section class="rounded-lg border border-border bg-surface p-4">
			<h2 class="mt-0 mb-3 text-base font-semibold">Rest of season</h2>
			<dl class="num m-0 grid grid-cols-2 gap-x-4 gap-y-3 text-sm">
				<div>
					<dt class="text-muted">Projected record</dt>
					<dd class="m-0 text-lg font-semibold">{o.expected_wins.toFixed(1)}–{o.expected_losses.toFixed(1)}</dd>
				</div>
				<div>
					<dt class="text-muted">Conference title game</dt>
					<dd class="m-0 text-lg font-semibold">{pct(o.conf_title_game)}</dd>
				</div>
				<div>
					<dt class="text-muted">Conference title</dt>
					<dd class="m-0 text-lg font-semibold">{pct(o.conf_title)}</dd>
				</div>
				<div>
					<dt class="text-muted">Makes playoff</dt>
					<dd class="m-0 text-lg font-semibold">{pct(o.playoff)}</dd>
				</div>
				<div>
					<dt class="text-muted">First-round bye</dt>
					<dd class="m-0 text-lg font-semibold">{pct(o.bye)}</dd>
				</div>
				<div>
					<dt class="text-muted">Wins title</dt>
					<dd class="m-0 text-lg font-semibold">{pct(o.title)}</dd>
				</div>
			</dl>
			<p class="mt-4 mb-0 text-xs text-muted">
				Schedule: average opponent {signed(r.schedule_strength)}; a #25 team would expect
				{r.top25_expected_losses.toFixed(1)} losses against it.
			</p>
		</section>
	{/if}
</div>

<section class="mt-6 overflow-x-auto rounded-lg border border-border bg-surface">
	<h2 class="m-0 px-4 pt-4 pb-2 text-base font-semibold">Schedule</h2>
	<table class="num w-full text-sm">
		<thead>
			<tr class="border-b border-border text-left text-xs text-muted">
				<th class="px-4 py-2 font-medium">Date</th>
				<th class="px-2 py-2 font-medium">Opponent</th>
				<th class="px-2 py-2 font-medium">Result</th>
				<th class="px-2 py-2 text-right font-medium" title="Pregame win probability">Win %</th>
				<th class="hidden px-2 py-2 text-right font-medium sm:table-cell">Model score</th>
				<th class="hidden px-4 py-2 text-right font-medium sm:table-cell">Market</th>
			</tr>
		</thead>
		<tbody>
			{#each t.schedule as g (g.id)}
				<tr class="border-b border-border last:border-0">
					<td class="px-4 py-2 whitespace-nowrap text-muted">{shortDate(g.date)}</td>
					<td class="px-2 py-2">
						<span class="inline-block w-4 text-faint">{g.site === 'away' ? '@' : g.site === 'neutral' ? 'vs' : ''}</span>
						<Logo id={g.opponent_logo_id} size={18} class="mr-1" />
						{#if g.opponent_rank && g.opponent_rank <= 25}<span class="text-xs text-faint">{g.opponent_rank}</span>{/if}
						{#if g.opponent_slug}<a href="/team/{g.opponent_slug}" class="no-underline hover:underline"
								>{g.opponent}</a
							>{:else}{g.opponent}{/if}
					</td>
					<td class="px-2 py-2 whitespace-nowrap">
						{#if g.result}
							<span class="font-semibold {g.result[0] > g.result[1] ? 'text-up' : 'text-down'}"
								>{g.result[0] > g.result[1] ? 'W' : 'L'}</span
							>
							{g.result[0]}–{g.result[1]}
						{:else}<span class="text-faint">—</span>{/if}
					</td>
					<td class="px-2 py-2 text-right">{pct(g.win_prob)}</td>
					<td class="hidden px-2 py-2 text-right text-muted sm:table-cell"
						>{Math.round(g.points_for)}–{Math.round(g.points_against)}</td
					>
					<td class="hidden px-4 py-2 text-right text-muted sm:table-cell"
						>{g.market_margin === null ? '' : signed(g.market_margin)}</td
					>
				</tr>
			{/each}
		</tbody>
	</table>
	<p class="m-0 px-4 py-3 text-xs text-faint">
		Win % for played games is the forecast made before kickoff. Market is the closing spread, shown as this team's
		expected margin.
	</p>
</section>
