<script lang="ts">
  import { fr } from '$lib/i18n/fr';
  import Badge from '$lib/components/ui/Badge.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import Modal from '$lib/components/ui/Modal.svelte';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
  import LoadingSpinner from '$lib/components/ui/LoadingSpinner.svelte';
  import ErrorBanner from '$lib/components/ui/ErrorBanner.svelte';
  import RuleForm from '$lib/components/rules/RuleForm.svelte';
  import { rules, rulesLoading, rulesError, fetchRules } from '$lib/stores/rules';
  import { createRule, deleteRule, toggleRule } from '$lib/api/client';
  import type { RuleMessage, RuleCriteria } from '$lib/types';

  // Modal states
  let showCreateModal = $state(false);
  let showEditModal = $state(false);
  let showDeleteModal = $state(false);
  let editingRule = $state<RuleMessage | null>(null);
  let deletingRule = $state<RuleMessage | null>(null);
  let saving = $state(false);
  let deleting = $state(false);

  function effectVariant(effect: string): 'green' | 'red' | 'orange' | 'purple' {
    if (effect === 'allow') return 'green';
    if (effect === 'block') return 'red';
    if (effect === 'ask') return 'orange';
    return 'purple';
  }

  function effectLabel(effect: string): string {
    if (effect === 'allow') return fr.rules_allow;
    if (effect === 'block') return fr.rules_block;
    if (effect === 'ask') return fr.rules_ask;
    return fr.rules_observe;
  }

  function sourceLabel(source: string): string {
    if (source === 'manual') return fr.rules_manual;
    if (source === 'auto_learning') return fr.rules_auto_learning;
    if (source === 'import') return fr.rules_import_source;
    return fr.rules_system;
  }

  function isSystemRule(rule: RuleMessage): boolean {
    return rule.source === 'system';
  }

  async function handleCreate(data: { name: string; priority: number; effect: string; criteria: RuleCriteria; source: string }) {
    saving = true;
    try {
      await createRule({
        name: data.name,
        priority: data.priority,
        effect: data.effect,
        criteria_json: JSON.stringify(data.criteria),
        scope_json: JSON.stringify({ type: 'permanent' }),
        source: data.source,
      });
      showCreateModal = false;
      await fetchRules();
    } catch (e) {
      console.error('Failed to create rule:', e);
    } finally {
      saving = false;
    }
  }

  async function handleEdit(data: { name: string; priority: number; effect: string; criteria: RuleCriteria; source: string }) {
    if (!editingRule) return;
    saving = true;
    try {
      // Delete + recreate since we have no update RPC
      await deleteRule(editingRule.id);
      await createRule({
        name: data.name,
        priority: data.priority,
        effect: data.effect,
        criteria_json: JSON.stringify(data.criteria),
        scope_json: JSON.stringify({ type: 'permanent' }),
        source: data.source,
      });
      showEditModal = false;
      editingRule = null;
      await fetchRules();
    } catch (e) {
      console.error('Failed to edit rule:', e);
    } finally {
      saving = false;
    }
  }

  async function handleDelete() {
    if (!deletingRule) return;
    deleting = true;
    try {
      await deleteRule(deletingRule.id);
      showDeleteModal = false;
      deletingRule = null;
      await fetchRules();
    } catch (e) {
      console.error('Failed to delete rule:', e);
    } finally {
      deleting = false;
    }
  }

  async function handleToggle(rule: RuleMessage) {
    try {
      await toggleRule(rule.id, !rule.enabled);
      await fetchRules();
    } catch (e) {
      console.error('Failed to toggle rule:', e);
    }
  }

  function openEdit(rule: RuleMessage) {
    editingRule = rule;
    showEditModal = true;
  }

  function openDelete(rule: RuleMessage) {
    deletingRule = rule;
    showDeleteModal = true;
  }

  // Sorted by priority
  const sortedRules = $derived([...$rules].sort((a, b) => a.priority - b.priority));
</script>

<div class="page-header">
  <h1 class="page-title">{fr.rules_title}</h1>
  <Button variant="primary" size="md" onclick={() => (showCreateModal = true)}>
    + {fr.rules_new}
  </Button>
</div>

{#if $rulesError}
  <ErrorBanner message={$rulesError} onretry={fetchRules} />
{/if}

{#if $rulesLoading}
  <LoadingSpinner />
{:else if sortedRules.length === 0}
  <EmptyState message={fr.rules_no_rules} />
{:else}
  <div class="rules-table">
    <div class="rules-header">
      <div class="rule-col col-priority">{fr.rules_priority}</div>
      <div class="rule-col col-name">{fr.rules_name}</div>
      <div class="rule-col col-effect">{fr.rules_effect}</div>
      <div class="rule-col col-source">{fr.rules_source}</div>
      <div class="rule-col col-status">{fr.rules_status}</div>
      <div class="rule-col col-actions">{fr.rules_actions}</div>
    </div>
    {#each sortedRules as rule (rule.id)}
      <div class="rules-row">
        <div class="rule-col col-priority font-mono">{rule.priority}</div>
        <div class="rule-col col-name">
          <span class="rule-name truncate">{rule.name}</span>
          {#if isSystemRule(rule)}
            <span class="lock-icon" title={fr.rules_system}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
                <path d="M7 11V7a5 5 0 0 1 10 0v4" />
              </svg>
            </span>
          {/if}
        </div>
        <div class="rule-col col-effect">
          <Badge variant={effectVariant(rule.effect)} label={effectLabel(rule.effect)} />
        </div>
        <div class="rule-col col-source text-secondary text-sm">
          {sourceLabel(rule.source)}
        </div>
        <div class="rule-col col-status">
          <button
            class="toggle-switch"
            class:active={rule.enabled}
            onclick={() => handleToggle(rule)}
            aria-label={rule.enabled ? fr.rules_enabled : fr.rules_disabled}
          >
            <span class="toggle-thumb"></span>
          </button>
        </div>
        <div class="rule-col col-actions">
          <button
            class="action-btn"
            onclick={() => openEdit(rule)}
            title={fr.rules_edit}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
              <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
            </svg>
          </button>
          <button
            class="action-btn danger"
            onclick={() => openDelete(rule)}
            disabled={isSystemRule(rule)}
            title={fr.rules_delete}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="3 6 5 6 21 6" />
              <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
            </svg>
          </button>
        </div>
      </div>
    {/each}
  </div>
{/if}

<!-- Create rule modal -->
<Modal open={showCreateModal} title={fr.rules_new} size="lg" onclose={() => (showCreateModal = false)}>
  <RuleForm
    onsave={handleCreate}
    oncancel={() => (showCreateModal = false)}
    {saving}
  />
</Modal>

<!-- Edit rule modal -->
<Modal open={showEditModal} title={fr.rules_edit} size="lg" onclose={() => { showEditModal = false; editingRule = null; }}>
  {#if editingRule}
    <RuleForm
      editRule={editingRule}
      onsave={handleEdit}
      oncancel={() => { showEditModal = false; editingRule = null; }}
      {saving}
    />
  {/if}
</Modal>

<!-- Delete confirmation modal -->
<Modal
  open={showDeleteModal}
  title={fr.rules_delete_confirm}
  size="sm"
  onclose={() => { showDeleteModal = false; deletingRule = null; }}
>
  <p class="delete-message">{fr.rules_delete_message}</p>
  {#if deletingRule}
    <p class="delete-rule-name font-mono">{deletingRule.name}</p>
  {/if}
  {#snippet footer()}
    <Button variant="ghost" onclick={() => { showDeleteModal = false; deletingRule = null; }}>
      {fr.rules_cancel}
    </Button>
    <Button variant="danger" loading={deleting} onclick={handleDelete}>
      {fr.rules_delete}
    </Button>
  {/snippet}
</Modal>

<style>
  .page-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .page-title {
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-semibold);
    color: var(--text-primary);
  }

  .rules-table {
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }

  .rules-header {
    display: flex;
    align-items: center;
    background: var(--bg-tertiary);
    border-bottom: 1px solid var(--border-primary);
    padding: var(--space-2) var(--space-4);
  }

  .rules-header .rule-col {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .rules-row {
    display: flex;
    align-items: center;
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--border-subtle);
    transition: background var(--transition-fast);
  }

  .rules-row:last-child {
    border-bottom: none;
  }

  .rules-row:hover {
    background: var(--bg-hover);
  }

  .rule-col {
    font-size: var(--font-size-sm);
  }

  .col-priority {
    width: 80px;
    flex-shrink: 0;
  }

  .col-name {
    flex: 2;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }

  .rule-name {
    min-width: 0;
  }

  .col-effect {
    width: 120px;
    flex-shrink: 0;
  }

  .col-source {
    width: 140px;
    flex-shrink: 0;
  }

  .col-status {
    width: 80px;
    flex-shrink: 0;
  }

  .col-actions {
    width: 100px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    justify-content: flex-end;
  }

  .lock-icon {
    color: var(--text-tertiary);
    flex-shrink: 0;
    display: flex;
    align-items: center;
  }

  /* Toggle switch */
  .toggle-switch {
    position: relative;
    width: 40px;
    height: 22px;
    background: var(--border-primary);
    border: none;
    border-radius: var(--radius-full);
    cursor: pointer;
    transition: background var(--transition-fast);
    padding: 0;
  }

  .toggle-switch.active {
    background: var(--accent-green);
  }

  .toggle-thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 18px;
    height: 18px;
    background: white;
    border-radius: 50%;
    transition: transform var(--transition-fast);
  }

  .toggle-switch.active .toggle-thumb {
    transform: translateX(18px);
  }

  /* Action buttons */
  .action-btn {
    background: none;
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    cursor: pointer;
    padding: var(--space-1);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all var(--transition-fast);
  }

  .action-btn:hover:not(:disabled) {
    color: var(--accent-cyan);
    border-color: var(--accent-cyan);
  }

  .action-btn.danger:hover:not(:disabled) {
    color: var(--accent-red);
    border-color: var(--accent-red);
  }

  .action-btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .delete-message {
    color: var(--text-secondary);
    font-size: var(--font-size-sm);
    margin-bottom: var(--space-3);
  }

  .delete-rule-name {
    color: var(--accent-red);
    font-size: var(--font-size-sm);
  }
</style>
