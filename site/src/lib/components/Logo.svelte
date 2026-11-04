<script lang="ts">
	// Team logo from the CollegeFootballData CDN, with its dark-surface
	// variant. Renders an empty box of the same size when there is no id or
	// the image fails, so rows stay aligned.
	let { id, size = 20, class: cls = '' }: { id: number | null | undefined; size?: number; class?: string } =
		$props();
	const px = $derived(size <= 24 ? 64 : size <= 48 ? 128 : 256);
	let failed = $state(false);
</script>

{#if id != null && !failed}
	<picture class="inline-block shrink-0 align-middle {cls}" style="width:{size}px;height:{size}px">
		<source media="(prefers-color-scheme: dark)" srcset="https://cdn.collegefootballdata.com/logos-dark/{px}/{id}.png" />
		<img
			src="https://cdn.collegefootballdata.com/logos/{px}/{id}.png"
			alt=""
			width={size}
			height={size}
			loading="lazy"
			decoding="async"
			class="h-full w-full object-contain"
			onerror={() => (failed = true)}
		/>
	</picture>
{:else}
	<span class="inline-block shrink-0 align-middle {cls}" style="width:{size}px;height:{size}px" aria-hidden="true"></span>
{/if}
