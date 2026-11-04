<script lang="ts">
	import type { WeekPoint } from '#lib/types.ts';
	import { signed } from '#lib/format.ts';

	// Rank after each week (best at the top) with its 90% rank range as a
	// band, and a hover readout per week.
	let { points, height = 180 }: { points: WeekPoint[]; height?: number } = $props();

	// Drawn at the container's pixel width so text stays legible on phones.
	let W = $state(640);
	const H = $derived(Math.min(height, Math.max(140, W * 0.55)));
	const pad = { l: 34, r: 44, t: 10, b: 24 };

	// Rank axis from #1 to just past the worst plausible rank, on nice steps.
	const worst = $derived(Math.max(...points.map((p) => p.rank_high)));
	const step = $derived(worst <= 12 ? 2 : worst <= 30 ? 5 : worst <= 60 ? 10 : 25);
	const yMax = $derived(Math.ceil(worst / step) * step);
	const ticks = $derived.by(() => {
		const t = [1];
		for (let v = step; v <= yMax; v += step) t.push(v);
		return t;
	});
	const x = (i: number) => pad.l + (i * (W - pad.l - pad.r)) / Math.max(1, points.length - 1);
	const y = (r: number) => pad.t + ((r - 1) * (H - pad.t - pad.b)) / Math.max(1, yMax - 1);

	const line = $derived(points.map((p, i) => `${i ? 'L' : 'M'}${x(i).toFixed(1)},${y(p.rank).toFixed(1)}`).join(''));
	const band = $derived(
		points.map((p, i) => `${i ? 'L' : 'M'}${x(i).toFixed(1)},${y(p.rank_low).toFixed(1)}`).join('') +
			points
				.map((p, i) => [i, p] as const)
				.reverse()
				.map(([i, p]) => `L${x(i).toFixed(1)},${y(p.rank_high).toFixed(1)}`)
				.join('') +
			'Z'
	);
	const short = (label: string) =>
		label === 'Preseason' ? 'Pre' : label.replace('Week ', 'W').replace('Postseason', 'Post');
	const labelEvery = $derived(Math.ceil(points.length / 8));

	let hover: number | null = $state(null);
	function move(e: PointerEvent) {
		const r = (e.currentTarget as SVGSVGElement).getBoundingClientRect();
		const px = ((e.clientX - r.left) / r.width) * W;
		const i = Math.round(((px - pad.l) / (W - pad.l - pad.r)) * (points.length - 1));
		hover = Math.max(0, Math.min(points.length - 1, i));
	}
	const last = $derived(points[points.length - 1]);
</script>

<div class="relative" bind:clientWidth={W}>
	<svg
		viewBox="0 0 {W} {H}"
		class="block h-auto w-full touch-none select-none"
		role="img"
		aria-label="Rank by week: #{points[0].rank} preseason, #{last.rank} now"
		onpointermove={move}
		onpointerleave={() => (hover = null)}
	>
		{#each ticks as t (t)}
			<line x1={pad.l} x2={W - pad.r} y1={y(t)} y2={y(t)} stroke="var(--grid)" stroke-width="1" />
			<text x={pad.l - 6} y={y(t) + 4} text-anchor="end" font-size="11" fill="var(--muted)" class="num">{t}</text>
		{/each}
		{#each points as p, i (i)}
			{#if i % labelEvery === 0 || i === points.length - 1}
				<text x={x(i)} y={H - 6} text-anchor="middle" font-size="11" fill="var(--muted)">{short(p.label)}</text>
			{/if}
		{/each}
		<path d={band} fill="var(--band-1)" />
		<path d={line} fill="none" stroke="var(--series-1)" stroke-width="2" stroke-linejoin="round" stroke-linecap="round" />
		{#each points as p, i (i)}
			<circle cx={x(i)} cy={y(p.rank)} r={i === points.length - 1 ? 4 : 2.5} fill="var(--series-1)" stroke="var(--surface)" stroke-width="1.5" />
		{/each}
		<text x={x(points.length - 1) + 8} y={y(last.rank) + 4} font-size="12" font-weight="600" fill="var(--text)" class="num"
			>#{last.rank}</text
		>
		{#if hover !== null}
			<line x1={x(hover)} x2={x(hover)} y1={pad.t} y2={H - pad.b} stroke="var(--faint)" stroke-width="1" />
			<circle cx={x(hover)} cy={y(points[hover].rank)} r="4.5" fill="var(--series-1)" stroke="var(--surface)" stroke-width="2" />
		{/if}
	</svg>
	{#if hover !== null}
		{@const p = points[hover]}
		<div
			class="pointer-events-none absolute top-0 z-10 rounded-md border border-border bg-surface px-2.5 py-1.5 text-xs shadow-sm"
			style="left: {(x(hover) / W) * 100}%; transform: translateX({hover > points.length / 2 ? '-105%' : '5%'})"
		>
			<div class="font-semibold">{p.label}</div>
			<div class="num">#{p.rank} <span class="text-faint">({p.rank_low}–{p.rank_high})</span></div>
			<div class="num text-muted">Rating {signed(p.overall)}</div>
		</div>
	{/if}
</div>
