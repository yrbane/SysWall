<script lang="ts">
  interface Props {
    label: string;
    value: string | number;
    icon?: string;
    color?: 'cyan' | 'green' | 'red' | 'orange' | 'purple';
  }

  let { label, value, icon, color = 'cyan' }: Props = $props();

  const colorVarMap: Record<string, string> = {
    cyan: 'var(--accent-cyan)',
    green: 'var(--accent-green)',
    red: 'var(--accent-red)',
    orange: 'var(--accent-orange)',
    purple: 'var(--accent-purple)',
  };

  const glowVarMap: Record<string, string> = {
    cyan: 'var(--glow-cyan)',
    green: 'var(--glow-green)',
    red: 'var(--glow-red)',
    orange: 'var(--glow-orange)',
    purple: 'var(--glow-purple)',
  };
</script>

<div class="stat-card" style="box-shadow: {glowVarMap[color]};">
  <div class="stat-content">
    <span class="stat-value" style="color: {colorVarMap[color]};">{value}</span>
    <span class="stat-label">{label}</span>
  </div>
  {#if icon}
    <div class="stat-icon" style="color: {colorVarMap[color]};">
      {@html icon}
    </div>
  {/if}
</div>

<style>
  .stat-card {
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-lg);
    padding: var(--space-4) var(--space-5);
    display: flex;
    align-items: center;
    justify-content: space-between;
    transition: transform 150ms ease, border-color 150ms ease, box-shadow 150ms ease;
  }

  .stat-card:hover {
    transform: translateY(-1px);
    border-color: rgba(255, 255, 255, 0.12);
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  }

  .stat-content {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .stat-value {
    font-family: var(--font-mono);
    font-size: var(--font-size-2xl);
    font-weight: var(--font-weight-bold);
    line-height: 1;
  }

  .stat-label {
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
    font-weight: var(--font-weight-medium);
  }

  .stat-icon {
    opacity: 0.6;
    flex-shrink: 0;
  }
</style>
