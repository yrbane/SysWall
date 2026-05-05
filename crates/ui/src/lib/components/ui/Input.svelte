<script lang="ts">
  interface Props {
    type?: 'text' | 'number' | 'search' | 'date';
    placeholder?: string;
    value?: string;
    label?: string;
    error?: string;
    disabled?: boolean;
    oninput?: (e: Event) => void;
  }

  let { type = 'text', placeholder = '', value = $bindable(''), label, error, disabled = false, oninput }: Props = $props();
</script>

<div class="input-group" class:has-error={!!error} class:is-disabled={disabled}>
  {#if label}
    <label class="input-label">{label}</label>
  {/if}
  <input
    class="input"
    {type}
    {placeholder}
    {disabled}
    aria-invalid={!!error}
    aria-describedby={error ? 'input-error' : undefined}
    bind:value
    {oninput}
  />
  {#if error}
    <span id="input-error" class="error-message" role="alert">{error}</span>
  {/if}
</div>

<style>
  .input-group {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .input-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .input {
    background: var(--bg-tertiary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    padding: var(--space-2) var(--space-3);
    color: var(--text-primary);
    font-family: var(--font-sans);
    font-size: var(--font-size-sm);
    transition: border-color var(--transition-fast);
    outline: none;
    width: 100%;
  }

  .input::placeholder {
    color: var(--text-tertiary);
  }

  .input:focus {
    border-color: var(--accent-cyan);
    box-shadow: 0 0 0 1px var(--accent-cyan);
  }

  .input[type='search'] {
    font-family: var(--font-mono);
  }

  /* Etat erreur / Error state */
  .input-group.has-error .input {
    border-color: var(--accent-red);
  }

  .error-message {
    color: var(--accent-red);
    font-size: var(--font-size-sm);
    margin-top: 4px;
    display: block;
  }

  /* Etat désactivé / Disabled state */
  .input-group.is-disabled .input {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
