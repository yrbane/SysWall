import type { Action } from 'svelte/action';

const FOCUSABLE_SELECTOR =
  'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * Svelte action: trap Tab cycling within the element and restore focus on destroy.
 *
 * Action Svelte : capture le cycle Tab à l'intérieur de l'élément et restitue
 * le focus à la fermeture.
 */
export const focusTrap: Action<HTMLElement> = (node) => {
  const previouslyFocused = document.activeElement as HTMLElement | null;

  const focusables = (): HTMLElement[] =>
    Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));

  const first = focusables()[0];
  first?.focus();

  function onKeydown(e: KeyboardEvent) {
    if (e.key !== 'Tab') return;
    const list = focusables();
    if (list.length === 0) return;
    const firstEl = list[0];
    const lastEl = list[list.length - 1];
    if (e.shiftKey && document.activeElement === firstEl) {
      lastEl.focus();
      e.preventDefault();
    } else if (!e.shiftKey && document.activeElement === lastEl) {
      firstEl.focus();
      e.preventDefault();
    }
  }

  node.addEventListener('keydown', onKeydown);

  return {
    destroy() {
      node.removeEventListener('keydown', onKeydown);
      previouslyFocused?.focus();
    },
  };
};
