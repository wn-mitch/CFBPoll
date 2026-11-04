<script lang="ts">
	import type { TrajectoryPoint } from '#lib/types.ts';
	import { signed } from '#lib/format.ts';

	// Week-by-week ratings: overall with its 90% band, plus offense and
	// defense. One axis (points vs. an average FBS team).
	let { points }: { points: TrajectoryPoint[] } = $props();

	// Drawn at the container's pixel width so text stays legible on phones.
	let W = $state(640);
	const H = 240;
	const pad = { l: 36, r: 104, t: 12, b: 26 };
	const z = 1.645;
	const series = [
		{ key: 'overall', label: 'Overall', color: 'var(--series-1)' },
		{ key: 'offense', label: 'Offense', color: 'var(--series-2)' },
		{ key: 'defense', label: 'Defense', color: 'var(--series-3)' }
	] as const;

	const lo = $derived(Math.min(0, ...points.flatMap((p) => [p.overall - z * p.overall_sd, p.offense, p.defense])));
	const hi = $derived(Math.max(0, ...points.flatMap((p) => [p.overall + z * p.overall_sd, p.offense, p.defense])));
	const yMin = $derived(Math.floor(lo / 5) * 5);
	const yMax = $derived(Math.ceil(hi / 5) * 5);
	const x = (i: number) => pad.l + (i * (W - pad.l - pad.r)) / Math.max(1, points.length - 1);
	const y = (v: number) => pad.t + ((yMax - v) * (H - pad.t - pad.b)) / Math.max(1, yMax - yMin);
	const ticks = $derived.by(() => {
		const step = yMax - yMin > 40 ? 10 : 5;
		const t: number[] = [];
		for (let v = yMin; v <= yMax; v += step) t.push(v);
		return t;
	});
	const line = (k: 'overall' | 'offense' | 'defense') =>
		points.map((p, i) => `${i ? 'L' : 'M'}${x(i).toFixed(1)},${y(p[k]).toFixed(1)}`).join('');
	const band = $derived(
		points.map((p, i) => `${i ? 'L' : 'M'}${x(i).toFixed(1)},${y(p.overall + z * p.overall_sd).toFixed(1)}`).join('') +
			points
				.map((p, i) => [i, p] as const)
				.reverse()
				.map(([i, p]) => `L${x(i).toFixed(1)},${y(p.overall - z * p.overall_sd).toFixed(1)}`)
				.join('') +
			'Z'
	);

	// End labels, nudged apart only when they would overlap.
	const ends = $derived.by(() => {
		const last = points[points.length - 1];
		const raw = series.map((s) => ({ ...s, v: last[s.key], y: y(last[s.key]) }));
		const sorted = [...raw].sort((a, b) => a.y - b.y);
		for (let i = 1; i < sorted.length; i++) {
			if (sorted[i].y - sorted[i - 1].y < 13) sorted[i].y = sorted[i - 1].y + 13;
		}
		return sorted;
	});

	let hover: number | null = $state(null);
	function move(e: PointerEvent) {
		const svg = e.currentTarget as SVGSVGElement;
		const r = svg.getBoundingClientRect();
		const px = ((e.clientX - r.left) / r.width) * W;
		const i = Math.round(((px - pad.l) / (W - pad.l - pad.r)) * (points.length - 1));
		hover = Math.max(0, Math.min(points.length - 1, i));
	}
	const labelEvery = $derived(Math.ceil(points.length / 8));
</script>

<figure class="m-0">
	<div class="mb-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted">
		{#each series as s (s.key)}
			<span class="inline-flex items-center gap-1.5"
				><span class="inline-block h-0.5 w-4 rounded" style="background: {s.color}"></span>{s.label}</span
			>
		{/each}
		<span class="inline-flex items-center gap-1.5"
			><span class="inline-block h-2.5 w-4 rounded-sm" style="background: var(--band-1)"></span>Overall, 90% range</span
		>
	</div>
	<div class="relative" bind:clientWidth={W}>
		<svg
			viewBox="0 0 {W} {H}"
			class="block h-auto w-full touch-none select-none"
			role="img"
			aria-label="Rating by week"
			onpointermove={move}
			onpointerleave={() => (hover = null)}
		>
			{#each ticks as t (t)}
				<line x1={pad.l} x2={W - pad.r} y1={y(t)} y2={y(t)} stroke={t === 0 ? 'var(--faint)' : 'var(--grid)'} stroke-width="1" />
				<text x={pad.l - 6} y={y(t) + 4} text-anchor="end" font-size="11" fill="var(--muted)" class="num">{signed(t, 0)}</text>
			{/each}
			{#each points as p, i (i)}
				{#if i % labelEvery === 0 || i === points.length - 1}
					<text x={x(i)} y={H - 8} text-anchor="middle" font-size="11" fill="var(--muted)"
						>{p.label === 'Preseason' ? 'Pre' : p.label.replace('Week ', 'W').replace('Postseason', 'Post')}</text
					>
				{/if}
			{/each}
			<path d={band} fill="var(--band-1)" />
			{#each series as s (s.key)}
				<path d={line(s.key)} fill="none" stroke={s.color} stroke-width="2" stroke-linejoin="round" stroke-linecap="round" />
			{/each}
			{#each ends as e (e.key)}
				<text x={W - pad.r + 8} y={e.y + 4} font-size="11" fill="var(--text)" class="num">{e.label} {signed(e.v)}</text>
			{/each}
			{#if hover !== null}
				<line x1={x(hover)} x2={x(hover)} y1={pad.t} y2={H - pad.b} stroke="var(--faint)" stroke-width="1" />
				{#each series as s (s.key)}
					<circle cx={x(hover)} cy={y(points[hover][s.key])} r="4" fill={s.color} stroke="var(--surface)" stroke-width="2" />
				{/each}
			{/if}
		</svg>
		{#if hover !== null}
			{@const p = points[hover]}
			<div
				class="pointer-events-none absolute top-0 rounded border border-border bg-surface px-2 py-1.5 text-xs shadow-sm"
				style="left: {(x(hover) / W) * 100}%; transform: translateX({hover > points.length / 2 ? '-105%' : '5%'})"
			>
				<div class="font-semibold">{p.label}</div>
				<div class="num">Overall {signed(p.overall)} ± {(z * p.overall_sd).toFixed(1)}</div>
				<div class="num">Offense {signed(p.offense)}</div>
				<div class="num">Defense {signed(p.defense)}</div>
			</div>
		{/if}
	</div>
	<details class="mt-2 text-xs text-muted">
		<summary class="cursor-pointer">Table view</summary>
		<table class="num mt-2 w-full text-left">
			<thead><tr class="text-faint"><th class="py-1 font-medium">Week</th><th class="font-medium">Overall</th><th class="font-medium">± 90%</th><th class="font-medium">Offense</th><th class="font-medium">Defense</th></tr></thead>
			<tbody>
				{#each points as p, i (i)}
					<tr class="border-t border-border"><td class="py-1">{p.label}</td><td>{signed(p.overall)}</td><td>{(z * p.overall_sd).toFixed(1)}</td><td>{signed(p.offense)}</td><td>{signed(p.defense)}</td></tr>
				{/each}
			</tbody>
		</table>
	</details>
</figure>
