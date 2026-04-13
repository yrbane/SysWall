// Pending decisions store — fed by real-time events.
// Ouvre un popup au premier plan (comme Lulu) quand une décision est requise.

import { writable, derived } from 'svelte/store';
import { listen, emit } from '@tauri-apps/api/event';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { listPendingDecisions } from '$lib/api/client';
import type { PendingDecisionMessage, DomainEventPayload } from '$lib/types';

export const pendingDecisions = writable<PendingDecisionMessage[]>([]);
export const decisionsError = writable<string | null>(null);
export const decisionsLoading = writable(true);

export const pendingCount = derived(pendingDecisions, ($d) => $d.length);
export const showDecisionOverlay = derived(pendingDecisions, ($d) => $d.length > 0);

// Index of the currently displayed decision in the queue
export const currentDecisionIndex = writable(0);

export const currentDecision = derived(
  [pendingDecisions, currentDecisionIndex],
  ([$decisions, $index]) => {
    if ($decisions.length === 0) return null;
    return $decisions[Math.min($index, $decisions.length - 1)] ?? null;
  }
);

export async function fetchPendingDecisions(): Promise<void> {
  decisionsLoading.set(true);
  decisionsError.set(null);
  try {
    const result = await listPendingDecisions();
    pendingDecisions.set(result);
    currentDecisionIndex.set(0);
  } catch (e) {
    decisionsError.set(String(e));
  } finally {
    decisionsLoading.set(false);
  }
}

// Ouvre une fenêtre popup au premier plan pour une décision en attente.
// Opens a foreground popup window for a pending decision.
let popupOpen = false;

async function openDecisionPopup(decision: PendingDecisionMessage): Promise<void> {
  if (popupOpen) {
    // Popup déjà ouvert, envoyer la décision via événement
    // Popup already open, send decision via event
    await emit('syswall://popup-show-decision', JSON.stringify(decision));
    return;
  }

  try {
    const popup = new WebviewWindow('decision-popup', {
      url: '/popup/decision',
      title: 'SysWall',
      width: 480,
      height: 560,
      center: true,
      alwaysOnTop: true,
      focus: true,
      resizable: false,
      minimizable: false,
      skipTaskbar: true,
    });

    popupOpen = true;

    // Attendre que la fenêtre soit prête, puis envoyer la décision
    // Wait for window ready, then send the decision
    popup.once('tauri://created', async () => {
      // Petit délai pour laisser le temps au listener de s'initialiser
      // Small delay to let the listener initialize
      setTimeout(async () => {
        await emit('syswall://popup-show-decision', JSON.stringify(decision));
      }, 200);
    });

    popup.once('tauri://destroyed', () => {
      popupOpen = false;
    });
  } catch {
    popupOpen = false;
  }
}

export function initDecisionListeners(): () => void {
  const unlisteners: (() => void)[] = [];

  listen<DomainEventPayload>('syswall://decision-required', (event) => {
    try {
      const decision: PendingDecisionMessage = JSON.parse(event.payload.payload_json);
      pendingDecisions.update((list) => [decision, ...list]);
      // Ouvrir le popup au premier plan
      openDecisionPopup(decision);
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  listen<DomainEventPayload>('syswall://decision-resolved', (event) => {
    try {
      const payload = JSON.parse(event.payload.payload_json);
      const id = payload.id || payload.decision_id || payload;
      pendingDecisions.update((list) => list.filter((d) => d.id !== id));
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  listen<DomainEventPayload>('syswall://decision-expired', (event) => {
    try {
      const payload = JSON.parse(event.payload.payload_json);
      const id = typeof payload === 'string' ? payload : payload.id || payload;
      pendingDecisions.update((list) => list.filter((d) => d.id !== id));
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  return () => {
    unlisteners.forEach((fn) => fn());
  };
}
