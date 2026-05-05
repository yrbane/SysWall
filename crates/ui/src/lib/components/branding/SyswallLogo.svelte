<script lang="ts">
  interface Props {
    size?: number;
    variant?: 'mark' | 'full';
    accent?: string;
  }
  let { size = 24, variant = 'mark', accent = 'var(--accent-syswall)' }: Props = $props();

  let width = $derived(variant === 'full' ? size * 4.5 : size);
  let viewBox = $derived(variant === 'full' ? '0 0 108 24' : '0 0 24 24');
</script>

<svg
  width={width}
  height={size}
  {viewBox}
  fill="none"
  xmlns="http://www.w3.org/2000/svg"
  aria-label="SysWall"
  role="img"
>
  <!-- Bouclier / Shield -->
  <path
    d="M12 2 L20 5 V12 C20 17 16.5 21 12 22 C7.5 21 4 17 4 12 V5 Z"
    stroke={accent}
    stroke-width="1.5"
    stroke-linejoin="round"
    fill="rgba(44, 212, 212, 0.06)"
  />
  <!-- Filet d'interception en diagonale / Diagonal interception line -->
  <line
    x1="6"
    y1="14"
    x2="18"
    y2="6"
    stroke={accent}
    stroke-width="1.5"
    stroke-linecap="round"
  />

  {#if variant === 'full'}
    <text
      x="28"
      y="17"
      font-family="Inter Variable, sans-serif"
      font-weight="600"
      font-size="14"
      fill="currentColor"
    >
      SysWall
    </text>
  {/if}
</svg>
