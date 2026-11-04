<script lang="ts">
	import Seo from '#lib/components/Seo.svelte';
	import { signed } from '#lib/format.ts';
	import Logo from '#lib/components/Logo.svelte';

	let { data } = $props();
	const all = $derived(data.rows.flatMap((r) => r.member_ratings));
	const min = $derived(Math.floor(Math.min(...all) / 5) * 5);
	const max = $derived(Math.ceil(Math.max(...all) / 5) * 5);
	const x = (v: number) => `${(100 * (v - min)) / (max - min)}%`;
	let hover: { team: string; v: number } | null = $state(null);
	const ticks = $derived.by(() => {
		const t: number[] = [];
		for (let v = min; v <= max; v += 10) t.push(v);
		return t;
	});
</script>

<Seo title="Conferences" description="FBS conferences ranked by their teams' ratings, with every member shown." />

<h1 class="mt-0 mb-1 text-2xl font-bold tracking-tight">Conferences</h1>
<p class="mt-0 mb-5 max-w-[70ch] text-sm text-muted">
	Ordered by average member rating. Each logo is a team, placed at its rating. Every conference starts the season at the same level; only
	this season's cross-conference games move it, so the ± shows how much those few games pin it down.
</p>

<div class="rounded-lg border border-border bg-surface">
	<div class="grid grid-cols-[9rem_1fr] gap-3 border-b border-border px-3 py-2 sm:grid-cols-[12rem_1fr_7rem]" aria-hidden="true">
		<div class="text-xs text-faint">Points vs. average FBS team</div>
		<div class="num relative h-4 text-xs text-faint">
			{#each ticks as t (t)}
				<span class="absolute -translate-x-1/2" style="left: {x(t)}">{signed(t, 0)}</span>
			{/each}
		</div>
	</div>
	{#each data.rows as c (c.name)}
		<div class="grid grid-cols-[9rem_1fr] items-center gap-3 border-b border-border px-3 py-3 last:border-0 sm:grid-cols-[12rem_1fr_7rem]">
			<div>
				<div class="font-medium">{c.name}</div>
				<div class="num text-xs text-faint">avg {signed(c.member_average)}</div>
			</div>
			<div class="relative h-7">
				<div class="absolute top-0 bottom-0 w-px bg-border" style="left: {x(0)}"></div>
				{#each c.members as m, i (m)}
					<a
						href="/team/{c.member_slugs[i]}"
						class="absolute top-1/2 -translate-x-1/2 -translate-y-1/2 hover:z-10 hover:scale-125"
						style="left: {x(c.member_ratings[i])}"
						aria-label="{m} {signed(c.member_ratings[i])}"
						onpointerenter={() => (hover = { team: m, v: c.member_ratings[i] })}
						onpointerleave={() => (hover = null)}
						><Logo id={c.member_logo_ids[i]} size={22} /></a
					>
				{/each}
			</div>
			<div class="num hidden text-right text-xs text-muted sm:block" title="Conference level from cross-conference games">
				level {signed(c.level.value)} ± {(1.645 * c.level.sd).toFixed(1)}
			</div>
		</div>
	{/each}
</div>
<div class="num mt-2 h-5 text-sm text-muted">{hover ? `${hover.team}: ${signed(hover.v)}` : ''}</div>

<details class="mt-4 text-sm">
	<summary class="cursor-pointer text-muted">Table view</summary>
	{#each data.rows as c (c.name)}
		<h2 class="mt-4 mb-1 text-base font-semibold">{c.name}</h2>
		<p class="num m-0 text-muted">
			{#each c.members as m, i (m)}<a href="/team/{c.member_slugs[i]}" class="no-underline hover:underline">{m}</a> {signed(c.member_ratings[i])}{i < c.members.length - 1 ? ' · ' : ''}{/each}
		</p>
	{/each}
</details>
