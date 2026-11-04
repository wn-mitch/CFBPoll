<script lang="ts">
	// 1200×630 card captured by scripts/og-image.ts as static/og.png.
	import { signed } from '#lib/format.ts';
	let { data } = $props();
</script>

<svelte:head><meta name="robots" content="noindex" /></svelte:head>

<div class="og fixed inset-0 z-50 flex flex-col bg-bg px-16 py-12" style="width:1200px;height:630px">
	<div class="flex items-baseline justify-between">
		<div class="text-5xl font-bold tracking-tight">CFB Poll</div>
		<div class="text-2xl text-muted">{data.meta.year} · {data.meta.as_of}</div>
	</div>
	<div class="mt-2 text-2xl text-muted">Who is actually best: ratings with honest uncertainty</div>
	<ol class="num mt-8 grid list-none grid-cols-2 gap-x-12 gap-y-4 p-0 text-3xl">
		{#each data.rows as r (r.slug)}
			<li class="flex items-baseline gap-4 border-b border-border pb-2">
				<span class="w-8 font-semibold text-muted">{r.rank}</span>
				<span class="flex-1 font-semibold">{r.team}</span>
				<span>{signed(r.overall.value)}</span>
			</li>
		{/each}
	</ol>
</div>
