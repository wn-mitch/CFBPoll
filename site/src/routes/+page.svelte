<script lang="ts">
	import { SvelteSet } from 'svelte/reactivity';
	import Seo from '#lib/components/Seo.svelte';
	import IntervalBar from '#lib/components/IntervalBar.svelte';
	import Logo from '#lib/components/Logo.svelte';
	import RankChart from '#lib/components/RankChart.svelte';
	import WeekLog from '#lib/components/WeekLog.svelte';
	import { record, signed } from '#lib/format.ts';
	import type { RankRow } from '#lib/types.ts';

	let { data } = $props();
	const z = 1.645;
	let conference = $state('All');
	const conferences = $derived([
		'All',
		...[...new Set(data.rows.map((r) => r.conference ?? 'Independent'))].sort((a, b) => a.localeCompare(b))
	]);
	const rows = $derived(
		conference === 'All' ? data.rows : data.rows.filter((r) => (r.conference ?? 'Independent') === conference)
	);
	const min = $derived(Math.floor(Math.min(...data.rows.map((r) => r.overall.value - z * r.overall.sd)) / 10) * 10);
	const max = $derived(Math.ceil(Math.max(...data.rows.map((r) => r.overall.value + z * r.overall.sd)) / 10) * 10);
	const axis = $derived.by(() => {
		const t: number[] = [];
		for (let v = Math.ceil(min / 40) * 40; v <= max; v += 40) t.push(v);
		return t;
	});
	const pos = (v: number) => `${(100 * (v - min)) / (max - min)}%`;
	const move = (r: RankRow) => (r.previous_rank === null ? 0 : r.previous_rank - r.rank);

	const open = new SvelteSet<string>();
	function toggle(slug: string) {
		if (open.has(slug)) open.delete(slug);
		else open.add(slug);
	}
	function rowClick(e: MouseEvent, slug: string) {
		// Links inside the row keep their own behavior; text selection does not toggle.
		if ((e.target as HTMLElement).closest('a, button')) return;
		if (window.getSelection()?.toString()) return;
		toggle(slug);
	}
</script>

<Seo />

<div class="mb-6 flex flex-wrap items-end justify-between gap-x-8 gap-y-4">
	<div>
		<h1 class="m-0 text-2xl font-bold tracking-tight">Who is actually best</h1>
		<p class="mt-1.5 mb-0 max-w-[64ch] text-sm leading-relaxed text-muted">
			Rating is the expected margin against an average FBS team on a neutral field; the bar is its 90% range.
			The rank range is where a team lands in 90% of plausible worlds. Click a row for its week-by-week movement. No preseason reputation, no conference bonus.
			<a href="/method" class="underline decoration-border underline-offset-2 hover:decoration-current">How it works</a>
		</p>
	</div>
	<label class="flex items-center gap-2 text-sm text-muted">
		Conference
		<select
			bind:value={conference}
			class="rounded-md border border-border bg-surface px-2 py-1.5 text-text hover:border-faint focus-visible:border-accent"
		>
			{#each conferences as c (c)}<option value={c}>{c}</option>{/each}
		</select>
	</label>
</div>

<div class="rounded-lg border border-border bg-surface">
	<table class="num w-full text-sm">
		<thead>
			<tr class="border-b border-border text-left text-xs text-muted">
				<th class="w-16 py-2.5 pr-1 pl-3 font-medium sm:w-20">Rank</th>
				<th class="px-2 py-2.5 font-medium">Team</th>
				<th class="hidden px-2 py-2.5 font-medium sm:table-cell">Record</th>
				<th class="px-2 py-2.5 text-right font-medium">Rating</th>
				<th class="hidden w-[20%] px-3 py-2.5 font-medium md:table-cell">
					<span class="sr-only">Rating with 90% range</span>
					<div class="relative h-3.5" aria-hidden="true">
						{#each axis as t (t)}
							<span class="absolute -translate-x-1/2 font-normal text-faint" style="left: {pos(t)}">{signed(t, 0)}</span>
						{/each}
					</div>
				</th>
				<th class="hidden px-2 py-2.5 text-right font-medium lg:table-cell">Off</th>
				<th class="hidden px-2 py-2.5 text-right font-medium lg:table-cell">Def</th>
				<th class="hidden px-2 py-2.5 font-medium xl:table-cell">Last game</th>
				<th class="hidden px-2 py-2.5 text-right font-medium lg:table-cell" title="Average opponent rating">Sched</th>
				<th class="w-9 py-2.5 pr-2"><span class="sr-only">Details</span></th>
			</tr>
		</thead>
		<tbody>
			{#each rows as r (r.slug)}
				{@const m = move(r)}
				{@const expanded = open.has(r.slug)}
				{#if conference === 'All' && r.rank === 26}
					<tr aria-hidden="true">
						<td colspan="10" class="border-b border-border bg-surface-2 px-3 py-1 text-[11px] font-medium tracking-wide text-faint uppercase">
							Outside the top 25
						</td>
					</tr>
				{/if}
				<tr
					class="group cursor-pointer border-b border-border transition-colors duration-150 last:border-0 hover:bg-surface-2 {expanded
						? 'bg-surface-2'
						: ''}"
					onclick={(e) => rowClick(e, r.slug)}
				>
					<td class="py-2.5 pr-1 pl-3 align-top">
						<div class="flex items-baseline gap-1.5">
							<span class="text-base leading-tight font-semibold">{r.rank}</span>
							{#if m !== 0}
								<span class="text-xs {m > 0 ? 'text-up' : 'text-down'}"
									><span aria-hidden="true">{m > 0 ? '▲' : '▼'}</span><span class="sr-only"
										>{m > 0 ? 'up' : 'down'}</span
									>{Math.abs(m)}</span
								>
							{/if}
						</div>
						<div class="text-xs text-faint" title="90% rank range">{r.rank_low}–{r.rank_high}</div>
					</td>
					<td class="px-2 py-2.5">
						<div class="flex items-center gap-2.5">
							<Logo id={r.logo_id} size={28} />
							<div class="min-w-0">
								<a href="/team/{r.slug}" class="font-medium no-underline hover:underline">{r.team}</a>
								<div class="text-xs text-faint">
									{r.conference ?? 'Independent'}<span class="sm:hidden">
										· {record(r.record.wins, r.record.losses)}</span
									>
								</div>
							</div>
						</div>
					</td>
					<td class="hidden px-2 py-2.5 whitespace-nowrap sm:table-cell">
						{record(r.record.wins, r.record.losses)}
						<span class="text-xs text-faint">({record(r.record.conf_wins, r.record.conf_losses)})</span>
					</td>
					<td class="px-2 py-2.5 text-right whitespace-nowrap">
						<div class="font-semibold">{signed(r.overall.value)}</div>
						<div class="text-xs text-faint">±{(z * r.overall.sd).toFixed(1)}</div>
					</td>
					<td class="hidden px-3 py-2.5 md:table-cell"><IntervalBar value={r.overall.value} sd={r.overall.sd} {min} {max} /></td>
					<td class="hidden px-2 py-2.5 text-right lg:table-cell">{signed(r.offense.value)}</td>
					<td class="hidden px-2 py-2.5 text-right lg:table-cell">{signed(r.defense.value)}</td>
					<td class="hidden max-w-56 truncate px-2 py-2.5 text-xs text-muted xl:table-cell">
						{#if r.last_game}
							{@const won = r.last_game.points_for > r.last_game.points_against}
							<span class="font-semibold {won ? 'text-up' : 'text-down'}">{won ? 'W' : 'L'}</span>
							{r.last_game.points_for}–{r.last_game.points_against}
							{r.last_game.site === 'away' ? '@' : 'vs'}
							{r.last_game.opponent}
						{/if}
					</td>
					<td class="hidden px-2 py-2.5 text-right lg:table-cell">{signed(r.schedule_strength)}</td>
					<td class="py-2.5 pr-2 text-right">
						<button
							type="button"
							class="inline-flex h-7 w-7 items-center justify-center rounded-md text-faint group-hover:text-muted hover:bg-border/60"
							aria-expanded={expanded}
							aria-controls="weeks-{r.slug}"
							aria-label="{expanded ? 'Hide' : 'Show'} week-by-week movement for {r.team}"
							onclick={() => toggle(r.slug)}
						>
							<svg viewBox="0 0 16 16" class="h-4 w-4 transition-transform duration-200 ease-out {expanded ? 'rotate-180' : ''}" aria-hidden="true"
								><path d="M4 6l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg
							>
						</button>
					</td>
				</tr>
				{#if expanded}
					<tr id="weeks-{r.slug}" class="border-b border-border bg-surface-2 last:border-0">
						<td colspan="10" class="px-3 pt-1 pb-4 sm:px-4">
							<div class="reveal grid gap-x-8 gap-y-4 lg:grid-cols-[1.25fr_1fr]">
								<section>
									<h3 class="m-0 mb-1 text-xs font-medium text-muted">
										Rank by week <span class="font-normal text-faint">· band is the 90% rank range</span>
									</h3>
									<RankChart points={r.history} height={250} />
								</section>
								<section>
									<h3 class="m-0 mb-1 flex items-baseline justify-between text-xs font-medium text-muted">
										Week by week
										<a href="/team/{r.slug}" class="font-medium text-accent no-underline hover:underline">Team page →</a>
									</h3>
									<WeekLog points={r.history} />
								</section>
							</div>
						</td>
					</tr>
				{/if}
			{/each}
		</tbody>
	</table>
</div>

<style>
	.reveal {
		animation: reveal 220ms cubic-bezier(0.25, 1, 0.5, 1);
	}
	@keyframes reveal {
		from {
			opacity: 0;
			transform: translateY(-4px);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.reveal {
			animation: none;
		}
	}
</style>
