<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import uPlot from 'uplot';
  import 'uplot/dist/uPlot.min.css';

  interface Props {
    data: { allowed: number; blocked: number }[];
    height?: number;
  }

  let { data, height = 200 }: Props = $props();

  let container: HTMLDivElement;
  let chart: uPlot | null = null;

  // Couleurs du thème / Theme colors
  const ALLOWED_COLOR = 'rgba(0, 255, 136, 0.8)';  // --accent-green
  const BLOCKED_COLOR = 'rgba(255, 68, 68, 0.7)';   // --accent-red
  const GRID_COLOR = 'rgba(255, 255, 255, 0.06)';
  const TEXT_COLOR = 'rgba(255, 255, 255, 0.4)';

  function buildData(points: typeof data): uPlot.AlignedData {
    const now = Math.floor(Date.now() / 1000);
    const timestamps = points.map((_, i) => now - (points.length - 1 - i));
    const allowed = points.map((p) => p.allowed);
    const blocked = points.map((p) => p.blocked);
    return [timestamps, allowed, blocked];
  }

  function createChart() {
    if (!container) return;
    const width = container.clientWidth;

    const opts: uPlot.Options = {
      width,
      height,
      cursor: { show: true },
      select: { show: false, left: 0, top: 0, width: 0, height: 0 },
      legend: { show: false },
      scales: {
        x: { time: true },
        y: { auto: true, range: [0, null] as any },
      },
      axes: [
        {
          stroke: TEXT_COLOR,
          grid: { stroke: GRID_COLOR, width: 1 },
          ticks: { stroke: GRID_COLOR, width: 1 },
          font: '10px monospace',
        },
        {
          stroke: TEXT_COLOR,
          grid: { stroke: GRID_COLOR, width: 1 },
          ticks: { stroke: GRID_COLOR, width: 1 },
          font: '10px monospace',
          size: 50,
        },
      ],
      series: [
        {},
        {
          label: 'Autorisé',
          stroke: ALLOWED_COLOR,
          fill: 'rgba(0, 255, 136, 0.1)',
          width: 2,
        },
        {
          label: 'Bloqué',
          stroke: BLOCKED_COLOR,
          fill: 'rgba(255, 68, 68, 0.1)',
          width: 2,
        },
      ],
    };

    chart = new uPlot(opts, buildData(data), container);
  }

  // Mise à jour quand les données changent
  // Update when data changes
  $effect(() => {
    if (chart && data) {
      chart.setData(buildData(data));
    }
  });

  onMount(() => {
    createChart();
  });

  onDestroy(() => {
    chart?.destroy();
  });
</script>

<div class="chart-container" bind:this={container}></div>

<style>
  .chart-container {
    width: 100%;
    background: var(--bg-tertiary);
    border-radius: var(--radius-md);
    padding: var(--space-2);
  }

  .chart-container :global(.u-wrap) {
    border-radius: var(--radius-sm);
  }
</style>
