// Store de notifications toast
// Toast notification store

import { writable } from 'svelte/store';

export interface ToastMessage {
  id: string;
  message: string;
  variant: 'success' | 'error' | 'warning' | 'info';
  duration?: number;
}

const { subscribe, update } = writable<ToastMessage[]>([]);

export const toasts = { subscribe };

/**
 * Ajoute une notification toast
 * Adds a toast notification
 */
export function addToast(
  message: string,
  variant: ToastMessage['variant'] = 'info',
  duration = 4000,
) {
  const id = crypto.randomUUID();
  update((all) => [...all, { id, message, variant, duration }]);
  if (duration > 0) {
    setTimeout(() => removeToast(id), duration);
  }
}

/**
 * Supprime une notification toast par son identifiant
 * Removes a toast notification by its id
 */
export function removeToast(id: string) {
  update((all) => all.filter((t) => t.id !== id));
}
