/**
 * Debounce a function: delays invocation until `delayMs` has passed without
 * a new call. Each call resets the timer.
 *
 * Diffère l'invocation jusqu'à ce que `delayMs` se soit écoulé sans nouvel appel.
 * Chaque appel réinitialise le timer.
 */
export function debounce<F extends (...args: never[]) => void>(
  fn: F,
  delayMs: number,
): F {
  let timer: ReturnType<typeof setTimeout> | null = null;
  return ((...args: Parameters<F>) => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => fn(...args), delayMs);
  }) as F;
}
