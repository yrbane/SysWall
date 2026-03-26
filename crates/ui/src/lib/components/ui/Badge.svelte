<script lang="ts">
  interface Props {
    variant?: 'cyan' | 'green' | 'red' | 'orange' | 'purple' | 'neutral';
    label: string;
    dot?: boolean;
  }

  let { variant = 'neutral', label, dot = false }: Props = $props();

  const colorMap: Record<string, { bg: string; fg: string }> = {
    cyan: { bg: 'var(--accent-cyan-15)', fg: 'var(--accent-cyan)' },
    green: { bg: 'var(--accent-green-15)', fg: 'var(--accent-green)' },
    red: { bg: 'var(--accent-red-15)', fg: 'var(--accent-red)' },
    orange: { bg: 'var(--accent-orange-15)', fg: 'var(--accent-orange)' },
    purple: { bg: 'var(--accent-purple-15)', fg: 'var(--accent-purple)' },
    neutral: { bg: 'rgba(139, 148, 158, 0.15)', fg: 'var(--text-secondary)' },
  };

  const colors = $derived(colorMap[variant] || colorMap.neutral);
</script>

<span
  class="badge"
  style="background: {colors.bg}; color: {colors.fg};"
>
  {#if dot}
    <span class="dot" style="background: {colors.fg};"></span>
  {/if}
  {label}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    padding: var(--space-1) var(--space-2);
    border-radius: var(--radius-full);
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    line-height: 1;
    white-space: nowrap;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }
</style>
