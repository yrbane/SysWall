// Pending decisions store — fed by real-time events.

import { writable, derived } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
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

export function initDecisionListeners(): () => void {
  const unlisteners: (() => void)[] = [];

  listen<DomainEventPayload>('syswall://decision-required', (event) => {
    try {
      const decision: PendingDecisionMessage = JSON.parse(event.payload.payload_json);
      pendingDecisions.update((list) => [decision, ...list]);
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
