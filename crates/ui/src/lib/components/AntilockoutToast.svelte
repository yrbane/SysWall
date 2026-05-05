<!--
  Composant toast critique affiché lors d'un rollback anti-lockout automatique.
  Critical toast component shown when an automatic anti-lockout rollback occurs.
-->
<script lang="ts">
  interface Props {
    rolledBackCount: number;
    onDismiss: () => void;
  }
  let { rolledBackCount, onDismiss }: Props = $props();
</script>

<div class="antilockout-toast" role="alert" aria-live="assertive">
  <div class="icon" aria-hidden="true">⚠</div>
  <div class="body">
    <div class="title">Mise à jour annulée — connectivité perdue</div>
    <div class="desc">
      {rolledBackCount} modification(s) de règle ont été annulées automatiquement.
      La connectivité réseau a été perdue après l'application.
    </div>
  </div>
  <button class="dismiss" onclick={onDismiss} aria-label="Fermer">×</button>
</div>

<style>
  .antilockout-toast {
    display: flex;
    gap: var(--space-3);
    padding: var(--space-4);
    border-radius: var(--radius-md);
    background: rgba(255, 69, 58, 0.12);
    border: 1px solid var(--accent-red);
    color: var(--text-primary);
    max-width: 480px;
  }
  .icon { font-size: 24px; color: var(--accent-red); }
  .title { font-weight: 600; margin-bottom: 4px; }
  .desc { font-size: var(--font-size-sm); color: var(--text-secondary); }
  .dismiss { background: none; border: 0; font-size: 20px; color: var(--text-secondary); cursor: pointer; }
</style>
