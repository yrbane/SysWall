<script lang="ts">
  import { fr } from '$lib/i18n/fr';
  import Badge from '$lib/components/ui/Badge.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import Card from '$lib/components/ui/Card.svelte';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
  import DecisionPrompt from '$lib/components/learning/DecisionPrompt.svelte';
  import {
    pendingDecisions,
    currentDecision,
    currentDecisionIndex,
    pendingCount,
  } from '$lib/stores/decisions';
  import { respondToDecision } from '$lib/api/client';
  import type { DecisionAction } from '$lib/types';

  let responding = $state(false);

  // History of responded decisions (local, session-only)
  let decisionHistory = $state<Array<{
    id: string;
    action: string;
    appName: string;
    timestamp: string;
  }>>([]);

  async function handleRespond(action: DecisionAction, granularity: string) {
    const decision = $currentDecision;
    if (!decision) return;

    responding = true;
    try {
      await respondToDecision({
        pending_decision_id: decision.id,
        action,
        granularity,
      });

      // Add to history
      let appName = fr.conn_unknown;
      try {
        const snap = JSON.parse(decision.snapshot_json);
        appName = snap.process_name || fr.conn_unknown;
      } catch {
        // ignore
      }

      decisionHistory = [
        {
          id: decision.id,
          action,
          appName,
          timestamp: new Date().toISOString(),
        },
        ...decisionHistory,
      ].slice(0, 20);
    } catch (e) {
      console.error('Failed to respond to decision:', e);
    } finally {
      responding = false;
    }
  }

  function navigateDecision(delta: number) {
    currentDecisionIndex.update((idx) => {
      const newIdx = idx + delta;
      if (newIdx < 0) return 0;
      if (newIdx >= $pendingDecisions.length) return $pendingDecisions.length - 1;
      return newIdx;
    });
  }

  function actionLabel(action: string): string {
    const map: Record<string, string> = {
      allow_once: fr.learn_allow_once,
      block_once: fr.learn_block_once,
      always_allow: fr.learn_always_allow,
      always_block: fr.learn_always_block,
      create_rule: fr.learn_create_rule,
      ignore: fr.learn_ignore,
    };
    return map[action] || action;
  }

  function actionVariant(action: string): 'green' | 'red' | 'orange' | 'purple' | 'neutral' {
    if (action.includes('allow')) return 'green';
    if (action.includes('block')) return 'red';
    if (action === 'create_rule') return 'purple';
    return 'neutral';
  }
</script>

<div class="page-header">
  <h1 class="page-title">{fr.learn_title}</h1>
  {#if $pendingCount > 0}
    <Badge variant="orange" label="{$pendingCount} {fr.dash_pending}" dot />
  {/if}
</div>

{#if $pendingDecisions.length > 0}
  <!-- Queue navigation -->
  {#if $pendingDecisions.length > 1}
    <div class="queue-nav">
      <Button
        variant="ghost"
        size="sm"
        disabled={$currentDecisionIndex <= 0}
        onclick={() => navigateDecision(-1)}
      >
        ← {fr.audit_previous}
      </Button>
      <span class="queue-counter text-sm text-secondary">
        {$currentDecisionIndex + 1} {fr.learn_decision_of} {$pendingDecisions.length}
      </span>
      <Button
        variant="ghost"
        size="sm"
        disabled={$currentDecisionIndex >= $pendingDecisions.length - 1}
        onclick={() => navigateDecision(1)}
      >
        {fr.audit_next} →
      </Button>
    </div>
  {/if}

  <!-- Current decision prompt -->
  {#if $currentDecision}
    <DecisionPrompt
      decision={$currentDecision}
      onrespond={handleRespond}
      {responding}
    />
  {/if}
{:else}
  <EmptyState message={fr.learn_no_pending} />
{/if}

<!-- Recent decision history -->
{#if decisionHistory.length > 0}
  <Card title={fr.learn_recent_decisions}>
    <div class="history-list">
      {#each decisionHistory as entry}
        <div class="history-item">
          <span class="history-app text-sm">{entry.appName}</span>
          <Badge variant={actionVariant(entry.action)} label={actionLabel(entry.action)} />
          <span class="history-time text-xs text-secondary font-mono">
            {new Date(entry.timestamp).toLocaleTimeString('fr-FR')}
          </span>
        </div>
      {/each}
    </div>
  </Card>
{/if}

<style>
  .page-header {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .page-title {
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-semibold);
    color: var(--text-primary);
  }

  .queue-nav {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-4);
  }

  .queue-counter {
    font-family: var(--font-mono);
  }

  .history-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .history-item {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--border-subtle);
  }

  .history-item:last-child {
    border-bottom: none;
  }

  .history-app {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .history-time {
    flex-shrink: 0;
  }
</style>
