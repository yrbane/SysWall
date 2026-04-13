<script lang="ts">
  import { fr } from '$lib/i18n/fr';
  import DecisionPrompt from '$lib/components/learning/DecisionPrompt.svelte';
  import { respondToDecision } from '$lib/api/client';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount, onDestroy } from 'svelte';
  import type { PendingDecisionMessage, DomainEventPayload, DecisionAction } from '$lib/types';
  import { addToast } from '$lib/stores/toast';

  let decision: PendingDecisionMessage | null = $state(null);
  let responding = $state(false);
  const unlisteners: (() => void)[] = [];

  onMount(async () => {
    // Écoute les décisions en attente — même événement que la fenêtre principale
    // Listen for pending decisions — same event as main window
    const unlisten1 = await listen<DomainEventPayload>('syswall://decision-required', (event) => {
      try {
        decision = JSON.parse(event.payload.payload_json);
      } catch { /* ignore */ }
    });
    unlisteners.push(unlisten1);

    // Écoute la résolution/expiration pour fermer le popup
    // Listen for resolution/expiry to close popup
    const unlisten2 = await listen<DomainEventPayload>('syswall://decision-resolved', async () => {
      decision = null;
      await closePopup();
    });
    unlisteners.push(unlisten2);

    const unlisten3 = await listen<DomainEventPayload>('syswall://decision-expired', async () => {
      decision = null;
      await closePopup();
    });
    unlisteners.push(unlisten3);

    // Écoute aussi l'événement direct du store principal
    // Also listen for direct event from main store
    const unlisten4 = await listen<string>('syswall://popup-show-decision', (event) => {
      try {
        decision = JSON.parse(event.payload);
      } catch { /* ignore */ }
    });
    unlisteners.push(unlisten4);
  });

  onDestroy(() => {
    unlisteners.forEach(fn => fn());
  });

  async function closePopup() {
    try {
      const win = getCurrentWindow();
      await win.close();
    } catch { /* ignore si déjà fermé */ }
  }

  async function handleRespond(action: DecisionAction, granularity: string) {
    if (!decision) return;
    responding = true;
    try {
      await respondToDecision({
        pending_decision_id: decision.id,
        action,
        granularity,
      });
      await closePopup();
    } catch (e) {
      addToast(String(e), 'error');
      responding = false;
    }
  }
</script>

<div class="popup-container">
  {#if decision}
    <DecisionPrompt
      {decision}
      onrespond={handleRespond}
      {responding}
    />
  {:else}
    <div class="popup-waiting">
      <p class="text-secondary">{fr.learn_waiting ?? 'En attente...'}</p>
    </div>
  {/if}
</div>

<style>
  .popup-container {
    padding: var(--space-4);
    height: 100vh;
    display: flex;
    flex-direction: column;
    justify-content: center;
    background: var(--bg-primary);
    overflow-y: auto;
  }

  .popup-waiting {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
  }
</style>
