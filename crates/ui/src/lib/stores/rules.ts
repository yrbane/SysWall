// Rules store — fetched on demand, updated by events.

import { writable, derived } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import { listRules } from '$lib/api/client';
import type { RuleMessage, DomainEventPayload } from '$lib/types';

export const rules = writable<RuleMessage[]>([]);
export const rulesError = writable<string | null>(null);
export const rulesLoading = writable(true);

export const rulesCount = derived(rules, ($r) => $r.length);
export const activeRulesCount = derived(rules, ($r) => $r.filter((r) => r.enabled).length);

export async function fetchRules(): Promise<void> {
  rulesLoading.set(true);
  rulesError.set(null);
  try {
    const result = await listRules();
    rules.set(result);
  } catch (e) {
    rulesError.set(String(e));
  } finally {
    rulesLoading.set(false);
  }
}

export function initRuleListeners(): () => void {
  const unlisteners: (() => void)[] = [];

  listen<DomainEventPayload>('syswall://rule-created', (event) => {
    try {
      const rule: RuleMessage = JSON.parse(event.payload.payload_json);
      rules.update((list) => [rule, ...list]);
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  listen<DomainEventPayload>('syswall://rule-updated', (event) => {
    try {
      const rule: RuleMessage = JSON.parse(event.payload.payload_json);
      rules.update((list) => list.map((r) => (r.id === rule.id ? rule : r)));
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  listen<DomainEventPayload>('syswall://rule-deleted', (event) => {
    try {
      const payload = JSON.parse(event.payload.payload_json);
      const id = typeof payload === 'string' ? payload : payload.id || payload;
      rules.update((list) => list.filter((r) => r.id !== id));
    } catch {
      // Ignore
    }
  }).then((fn) => unlisteners.push(fn));

  return () => {
    unlisteners.forEach((fn) => fn());
  };
}
