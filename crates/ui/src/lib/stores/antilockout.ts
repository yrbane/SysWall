// Store anti-lockout — écoute l'événement domaine et déclenche un toast critique.
// Antilockout store — listens to the domain event and fires a critical toast.

import { listen } from '@tauri-apps/api/event';
import { addToast } from '$lib/stores/toast';
import type { DomainEventPayload } from '$lib/types';

interface AntilockoutPayload {
  rolled_back_count: number;
}

/**
 * Initialise l'écouteur d'événements anti-lockout.
 * Initializes the antilockout event listener.
 *
 * Returns an unsubscribe function.
 */
export function initAntilockoutListener(): () => void {
  let unlisten: (() => void) | undefined;

  listen<DomainEventPayload>('syswall://antilockout-triggered', (event) => {
    try {
      const payload: AntilockoutPayload = JSON.parse(event.payload.payload_json);
      const count = payload.rolled_back_count ?? 0;
      addToast(
        `Mise à jour annulée — connectivité perdue. ${count} modification(s) de règle annulée(s) automatiquement.`,
        'error',
        0, // persistant jusqu'à fermeture manuelle
      );
    } catch {
      // Ignore parse errors
    }
  }).then((fn) => {
    unlisten = fn;
  });

  return () => {
    unlisten?.();
  };
}
