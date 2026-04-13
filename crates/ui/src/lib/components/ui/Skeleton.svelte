<!--
  Composant de chargement squelette avec animation shimmer
  Skeleton loading component with shimmer animation
-->
<script lang="ts">
  interface Props {
    lines?: number;
    height?: string;
  }

  let { lines = 3, height = '1rem' }: Props = $props();

  // Largeur décroissante par ligne / Decreasing width per line
  function lineWidth(index: number): string {
    const base = 90;
    const step = 10;
    const width = Math.max(base - index * step, 40);
    return `${width}%`;
  }
</script>

<div class="skeleton-wrapper" role="status" aria-label="Chargement">
  {#each Array(lines) as _, i}
    <div
      class="skeleton-line"
      style="width: {lineWidth(i)}; height: {height}"
    ></div>
  {/each}
</div>

<style>
  .skeleton-wrapper {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .skeleton-line {
    background: linear-gradient(
      90deg,
      var(--bg-tertiary) 25%,
      var(--bg-hover) 50%,
      var(--bg-tertiary) 75%
    );
    background-size: 200% 100%;
    border-radius: var(--radius-md);
    animation: shimmer 1.5s infinite;
  }

  @keyframes shimmer {
    0% { background-position: 200% 0; }
    100% { background-position: -200% 0; }
  }
</style>
