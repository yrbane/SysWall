<script lang="ts">
  import { fr } from '$lib/i18n/fr';
  import Badge from '$lib/components/ui/Badge.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import Card from '$lib/components/ui/Card.svelte';
  import type { PendingDecisionMessage, ConnectionSnapshot, DecisionAction } from '$lib/types';
  import { onMount, onDestroy } from 'svelte';

  interface Props {
    decision: PendingDecisionMessage;
    onrespond: (action: DecisionAction, granularity: string) => void;
    responding?: boolean;
  }

  let { decision, onrespond, responding = false }: Props = $props();

  // Parse the snapshot from JSON
  const snapshot: ConnectionSnapshot | null = $derived.by(() => {
    try {
      return JSON.parse(decision.snapshot_json);
    } catch {
      return null;
    }
  });

  // Countdown timer
  let remainingSeconds = $state(0);
  let countdownInterval: ReturnType<typeof setInterval> | undefined;

  function updateCountdown() {
    const expiresAt = new Date(decision.expires_at).getTime();
    const now = Date.now();
    remainingSeconds = Math.max(0, Math.floor((expiresAt - now) / 1000));
  }

  onMount(() => {
    updateCountdown();
    countdownInterval = setInterval(updateCountdown, 1000);
  });

  onDestroy(() => {
    if (countdownInterval) clearInterval(countdownInterval);
  });

  // Urgency level based on remaining time
  const urgency = $derived.by(() => {
    if (remainingSeconds <= 5) return 'critical';
    if (remainingSeconds <= 15) return 'warning';
    return 'normal';
  });

  const urgencyBorderColor = $derived.by(() => {
    if (urgency === 'critical') return 'var(--accent-red)';
    if (urgency === 'warning') return 'var(--accent-orange)';
    return 'var(--accent-cyan)';
  });

  function formatCountdown(secs: number): string {
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return m > 0 ? `${m}:${s.toString().padStart(2, '0')}` : `${s}s`;
  }

  function handleAction(action: DecisionAction) {
    // Default granularity: app_and_destination for persistent actions, app_only for one-time
    let granularity = 'app_and_destination';
    if (action === 'allow_once' || action === 'block_once' || action === 'ignore') {
      granularity = 'app_only';
    }
    onrespond(action, granularity);
  }
</script>

<div
  class="decision-prompt"
  class:urgency-warning={urgency === 'warning'}
  class:urgency-critical={urgency === 'critical'}
  style="border-color: {urgencyBorderColor};"
>
  <!-- Header with countdown -->
  <div class="prompt-header">
    <div class="prompt-title">
      <Badge variant="orange" label={fr.learn_new_connection} dot />
    </div>
    <div class="countdown" class:critical={urgency === 'critical'}>
      <span class="countdown-label text-xs text-secondary">{fr.learn_expires_in}</span>
      <span class="countdown-value font-mono">
        {formatCountdown(remainingSeconds)}
      </span>
    </div>
  </div>

  <!-- Connection details -->
  {#if snapshot}
    <div class="connection-info">
      <div class="app-header">
        <div class="app-icon">
          <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="var(--accent-cyan)" stroke-width="1.5">
            <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
            <line x1="8" y1="21" x2="16" y2="21" />
            <line x1="12" y1="17" x2="12" y2="21" />
          </svg>
        </div>
        <div class="app-details">
          <span class="app-name">{snapshot.process_name || fr.conn_unknown}</span>
          {#if snapshot.process_path}
            <span class="app-path font-mono text-xs text-secondary">{snapshot.process_path}</span>
          {/if}
        </div>
      </div>

      <div class="detail-grid">
        <div class="detail-item">
          <span class="detail-label">{fr.conn_user}</span>
          <span class="detail-value">{snapshot.user || '--'}</span>
        </div>
        <div class="detail-item">
          <span class="detail-label">{fr.learn_destination}</span>
          <span class="detail-value font-mono">{snapshot.destination?.ip}:{snapshot.destination?.port}</span>
        </div>
        <div class="detail-item">
          <span class="detail-label">{fr.conn_protocol}</span>
          <span class="detail-value">
            <Badge variant="cyan" label={snapshot.protocol.toUpperCase()} />
          </span>
        </div>
        <div class="detail-item">
          <span class="detail-label">{fr.conn_direction}</span>
          <span class="detail-value">
            {snapshot.direction === 'inbound' ? fr.conn_inbound : fr.conn_outbound}
          </span>
        </div>
      </div>
    </div>
  {/if}

  <!-- Action buttons -->
  <div class="actions">
    <div class="actions-row">
      <Button variant="success" size="sm" disabled={responding} onclick={() => handleAction('allow_once')}>
        {fr.learn_allow_once}
      </Button>
      <Button variant="danger" size="sm" disabled={responding} onclick={() => handleAction('block_once')}>
        {fr.learn_block_once}
      </Button>
    </div>
    <div class="actions-row">
      <Button variant="success" size="sm" disabled={responding} onclick={() => handleAction('always_allow')}>
        {fr.learn_always_allow}
      </Button>
      <Button variant="danger" size="sm" disabled={responding} onclick={() => handleAction('always_block')}>
        {fr.learn_always_block}
      </Button>
    </div>
    <div class="actions-row">
      <Button variant="primary" size="sm" disabled={responding} onclick={() => handleAction('create_rule')}>
        {fr.learn_create_rule}
      </Button>
      <Button variant="ghost" size="sm" disabled={responding} onclick={() => handleAction('ignore')}>
        {fr.learn_ignore}
      </Button>
    </div>
  </div>
</div>

<style>
  .decision-prompt {
    background: var(--bg-secondary);
    border: 2px solid var(--accent-cyan);
    border-radius: var(--radius-lg);
    padding: var(--space-4);
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    transition: border-color var(--transition-base), box-shadow var(--transition-base);
  }

  .decision-prompt.urgency-warning {
    box-shadow: var(--glow-orange);
  }

  .decision-prompt.urgency-critical {
    box-shadow: var(--glow-red);
    animation: urgentPulse 1s infinite;
  }

  @keyframes urgentPulse {
    0%, 100% { box-shadow: var(--glow-red); }
    50% { box-shadow: 0 0 24px rgba(255, 68, 68, 0.6); }
  }

  .prompt-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .prompt-title {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .countdown {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .countdown-value {
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-bold);
    color: var(--accent-orange);
  }

  .countdown.critical .countdown-value {
    color: var(--accent-red);
  }

  .connection-info {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .app-header {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .app-icon {
    flex-shrink: 0;
    width: 48px;
    height: 48px;
    background: var(--bg-tertiary);
    border-radius: var(--radius-md);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .app-details {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 0;
  }

  .app-name {
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
    color: var(--text-primary);
  }

  .app-path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .detail-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: var(--space-3);
  }

  .detail-item {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .detail-label {
    font-size: var(--font-size-xs);
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .detail-value {
    font-size: var(--font-size-sm);
    color: var(--text-primary);
  }

  .actions {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .actions-row {
    display: flex;
    gap: var(--space-2);
  }

  .actions-row > :global(*) {
    flex: 1;
  }
</style>
