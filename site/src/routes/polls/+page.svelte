<script lang="ts">
	import Seo from '#lib/components/Seo.svelte';
	import { signed } from '#lib/format.ts';

	let { data } = $props();
	const b = $derived(data.bias);
	let poll = $state('AP Top 25');
	const stats = $derived(b ? (b.by_conference[poll] ?? []) : []);
	const maxGap = $derived(Math.max(1, ...stats.map((s) => Math.abs(s.mean_gap))));
	const weeks = [2, 4, 6, 8, 10, 12, 14];
	const at = (series: [number, { mean_gap: number; n: number }][], w: number) => series.find(([x]) => x === w)?.[1];
</script>

<Seo title="vs. the polls" description="How the AP poll, Coaches Poll and CFP committee rankings compare with a model that ignores reputation." />

<h1 class="mt-0 mb-1 text-2xl font-bold tracking-tight">This model vs. the human polls</h1>
{#if !b}
	<p class="text-muted">The audit has not been generated yet.</p>
{:else}
	<p class="mt-0 mb-6 max-w-[72ch] text-sm text-muted">
		{b.comparisons.toLocaleString()} poll entries from {b.seasons[0]}–{b.seasons[b.seasons.length - 1]}, each compared with
		this model's rank using only games played before that poll came out. The gap is model rank minus poll rank:
		positive means the poll ranks the team higher than the model does. Every group runs positive because a poll's top
		25 is, by selection, the teams it likes most; the comparison that matters is between groups.
	</p>

	<section class="mb-10">
		<div class="mb-3 flex flex-wrap items-center gap-2">
			<h2 class="m-0 mr-2 text-lg font-semibold">By conference</h2>
			{#each Object.keys(b.by_conference) as p (p)}
				<button
					class="rounded border px-2 py-0.5 text-sm {poll === p ? 'border-accent bg-surface-2 font-semibold' : 'border-border text-muted'}"
					onclick={() => (poll = p)}>{p.replace('Playoff Committee Rankings', 'CFP committee')}</button
				>
			{/each}
		</div>
		<div class="rounded-lg border border-border bg-surface p-3">
			{#each stats as s (s.key)}
				<div class="grid grid-cols-[9rem_1fr_9rem] items-center gap-3 py-1 text-sm sm:grid-cols-[11rem_1fr_17rem]">
					<div class="truncate">{s.key}</div>
					<div class="relative h-4">
						<div
							class="absolute top-0 bottom-0 rounded-r"
							style="left: 0; width: {(100 * Math.max(0, s.mean_gap)) / maxGap}%; background: var(--series-1)"
						></div>
					</div>
					<div class="num text-right text-xs text-muted">
						{signed(s.mean_gap)} spots · n={s.n} · {Math.round(100 * s.above_range)}% outside range
					</div>
				</div>
			{/each}
			<p class="mt-2 mb-0 text-xs text-faint">
				"Outside range": the poll ranked the team better than the model's 90% rank range at the time.
			</p>
		</div>
	</section>

	<section class="mb-10">
		<h2 class="mt-0 mb-1 text-lg font-semibold">Preseason anchoring (AP)</h2>
		<p class="mt-0 mb-3 max-w-[72ch] text-sm text-muted">
			Mean gap by where a team started in the preseason AP poll. If voters anchor on preseason expectations, the
			preseason-favorite rows stay high while the evidence piles up.
		</p>
		<div class="overflow-x-auto rounded-lg border border-border bg-surface">
			<table class="num w-full text-sm">
				<thead>
					<tr class="border-b border-border text-left text-xs text-muted">
						<th class="px-3 py-2 font-medium">Preseason AP</th>
						{#each weeks as w (w)}<th class="px-2 py-2 text-right font-medium">Wk {w}</th>{/each}
					</tr>
				</thead>
				<tbody>
					{#each b.anchoring as [bucket, series] (bucket)}
						<tr class="border-b border-border last:border-0">
							<td class="px-3 py-2">{bucket.replace('preseason ', '')}</td>
							{#each weeks as w (w)}
								{@const s = at(series, w)}
								<td class="px-2 py-2 text-right">{s ? signed(s.mean_gap) : ''}</td>
							{/each}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	</section>

	<section class="mb-10">
		<h2 class="mt-0 mb-1 text-lg font-semibold">Left out</h2>
		<p class="mt-0 mb-3 text-sm text-muted">Poll weeks where a model top-10 team was missing from the AP top 25, by conference.</p>
		<p class="num m-0 text-sm">
			{#each b.snubs_by_conference as s, i (s.key)}{s.key} {s.count}{i < b.snubs_by_conference.length - 1 ? ' · ' : ''}{/each}
		</p>
	</section>

	{#if b.latest.length}
		<section>
			<h2 class="mt-0 mb-3 text-lg font-semibold">Latest AP poll vs. the model</h2>
			<div class="overflow-x-auto rounded-lg border border-border bg-surface">
				<table class="num w-full text-sm">
					<thead>
						<tr class="border-b border-border text-left text-xs text-muted">
							<th class="px-3 py-2 font-medium">AP</th>
							<th class="px-2 py-2 font-medium">Team</th>
							<th class="px-2 py-2 text-right font-medium">Model</th>
							<th class="px-3 py-2 text-right font-medium">Model range</th>
						</tr>
					</thead>
					<tbody>
						{#each b.latest as c (c.team)}
							<tr class="border-b border-border last:border-0">
								<td class="px-3 py-1.5">{c.poll_rank}</td>
								<td class="px-2 py-1.5">{c.team} <span class="text-xs text-faint">{c.conference}</span></td>
								<td class="px-2 py-1.5 text-right {c.poll_rank < c.model_low ? 'text-down' : c.poll_rank > c.model_high ? 'text-up' : ''}">{c.model_rank}</td>
								<td class="px-3 py-1.5 text-right text-muted">{c.model_low}–{c.model_high}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		</section>
	{/if}
{/if}
