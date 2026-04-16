<script lang="ts">
  import { fr } from '$lib/i18n/fr';
  import DecisionPrompt from '$lib/components/learning/DecisionPrompt.svelte';
  import { respondToDecision } from '$lib/api/client';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount, onDestroy } from 'svelte';
  import type { PendingDecisionMessage, DomainEventPayload, DecisionAction } from '$lib/types';

  let decision: PendingDecisionMessage | null = $state(null);
  let responding = $state(false);
  let debugMsg = $state('En attente de la décision...');
  const unlisteners: (() => void)[] = [];

  onMount(async () => {
    console.log('[Popup] Initialisé, en attente des événements...');

    // Écoute l'événement direct envoyé par le store principal
    // Listen for direct event sent by the main store
    const unlisten1 = await listen('syswall://popup-show-decision', (event: any) => {
      console.log('[Popup] Reçu popup-show-decision:', typeof event.payload, event.payload);
      try {
        // Le payload peut être une string JSON ou un objet
        // The payload can be a JSON string or an object
        const data = typeof event.payload === 'string' ? JSON.parse(event.payload) : event.payload;
        decision = data;
        debugMsg = `Décision reçue: ${decision?.id?.slice(0, 8)}`;
        console.log('[Popup] Décision parsée:', decision);
      } catch (e) {
        console.error('[Popup] Erreur parsing:', e);
        debugMsg = `Erreur: ${e}`;
      }
    });
    unlisteners.push(unlisten1);

    // Écoute aussi l'événement standard du daemon (au cas où)
    // Also listen for standard daemon event (just in case)
    const unlisten2 = await listen<DomainEventPayload>('syswall://decision-required', (event) => {
      console.log('[Popup] Reçu decision-required:', event.payload);
      if (!decision) {
        try {
          decision = JSON.parse(event.payload.payload_json);
        } catch { /* ignore */ }
      }
    });
    unlisteners.push(unlisten2);

    // Fermer le popup si la décision est résolue ou expirée
    // Close popup if decision is resolved or expired
    const unlisten3 = await listen('syswall://decision-resolved', async () => {
      decision = null;
      await closePopup();
    });
    unlisteners.push(unlisten3);

    const unlisten4 = await listen('syswall://decision-expired', async () => {
      decision = null;
      await closePopup();
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
    } catch { /* ignore si déjà fermé / ignore if already closed */ }
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
      <div class="waiting-icon">🛡️</div>
      <p class="waiting-text">{debugMsg}</p>
    </div>
  {/if}
</div>

<style>
  .popup-container {
    padding: 16px;
    height: 100vh;
    display: flex;
    flex-direction: column;
    justify-content: center;
    background: var(--bg-primary);
    overflow-y: auto;
  }

  .popup-waiting {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 12px;
  }

  .waiting-icon {
    font-size: 48px;
    opacity: 0.3;
  }

  .waiting-text {
    color: var(--text-secondary);
    font-size: 13px;
  }
</style>
