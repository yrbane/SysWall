<script lang="ts">
  import { page } from '$app/stores';
  import Badge from './Badge.svelte';
  import { fr } from '$lib/i18n/fr';

  interface NavItem {
    label: string;
    route: string;
    icon: string;
    badge?: number;
    pulsing?: boolean;
  }

  interface Props {
    firewallEnabled: boolean;
    items: NavItem[];
  }

  let { firewallEnabled, items }: Props = $props();

  const currentPath = $derived($page.url.pathname);
</script>

<nav class="sidebar" aria-label="Navigation principale">
  <div class="sidebar-header">
    <div class="logo">
      <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="var(--accent-cyan)" stroke-width="2">
        <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
      </svg>
      <span class="logo-text">SysWall</span>
    </div>
    <div class="status-indicator">
      <span class="status-dot" class:active={firewallEnabled}></span>
      <span class="status-text">
        {firewallEnabled ? fr.status_active : fr.status_inactive}
      </span>
    </div>
  </div>

  <div class="nav-items">
    {#each items as item}
      <a
        href={item.route}
        class="nav-item"
        class:active={currentPath === item.route || currentPath.startsWith(item.route + '/')}
        aria-current={currentPath === item.route ? 'page' : undefined}
      >
        <span class="nav-icon">
          {@html item.icon}
        </span>
        <span class="nav-label">{item.label}</span>
        {#if item.badge && item.badge > 0}
          <span class="nav-badge" class:pulsing={item.pulsing}>
            <Badge variant="cyan" label={String(item.badge)} />
          </span>
        {/if}
      </a>
    {/each}
  </div>
</nav>

<style>
  .sidebar {
    width: var(--sidebar-width);
    height: 100vh;
    background: var(--bg-secondary);
    border-right: 1px solid var(--border-primary);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    position: fixed;
    top: 0;
    left: 0;
    z-index: 50;
  }

  .sidebar-header {
    padding: var(--space-5) var(--space-4);
    border-bottom: 1px solid var(--border-primary);
  }

  .logo {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-bottom: var(--space-3);
  }

  .logo-text {
    font-family: var(--font-sans);
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-bold);
    color: var(--accent-cyan);
    letter-spacing: 0.02em;
  }

  .status-indicator {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent-red);
    flex-shrink: 0;
  }

  .status-dot.active {
    background: var(--accent-green);
    box-shadow: var(--glow-green);
  }

  .status-text {
    font-size: var(--font-size-xs);
    color: var(--text-secondary);
  }

  .nav-items {
    flex: 1;
    padding: var(--space-3) 0;
    overflow-y: auto;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    color: var(--text-secondary);
    text-decoration: none;
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    border-left: 3px solid transparent;
    transition: all var(--transition-fast);
    margin: var(--space-1) 0;
  }

  .nav-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .nav-item.active {
    border-left-color: var(--accent-cyan);
    background: var(--bg-hover);
    color: var(--accent-cyan);
  }

  .nav-icon {
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .nav-label {
    flex: 1;
  }

  .nav-badge {
    flex-shrink: 0;
  }

  .nav-badge.pulsing {
    animation: pulse 2s infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }

  /* --- Tablette : icônes seules / Tablet: icons only --- */
  @media (min-width: 640px) and (max-width: 1024px) {
    .sidebar {
      width: 60px;
    }

    .sidebar-header {
      padding: var(--space-3);
      display: flex;
      flex-direction: column;
      align-items: center;
    }

    .logo-text {
      display: none;
    }

    .status-text {
      display: none;
    }

    .nav-item {
      justify-content: center;
      padding: var(--space-3);
    }

    .nav-label {
      display: none;
    }

    .nav-badge {
      position: absolute;
      top: 2px;
      right: 4px;
      transform: scale(0.8);
    }

    .nav-item {
      position: relative;
    }
  }

  /* --- Mobile : barre de navigation en bas / Mobile: bottom tab bar --- */
  @media (max-width: 639px) {
    .sidebar {
      width: 100%;
      height: auto;
      position: fixed;
      top: auto;
      bottom: 0;
      left: 0;
      right: 0;
      flex-direction: row;
      border-right: none;
      border-top: 1px solid var(--border-primary);
      z-index: 100;
    }

    .sidebar-header {
      display: none;
    }

    .nav-items {
      display: flex;
      flex-direction: row;
      padding: 0;
      overflow-x: auto;
      overflow-y: hidden;
    }

    .nav-item {
      flex: 1;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      gap: var(--space-1);
      padding: var(--space-2) var(--space-1);
      border-left: none;
      margin: 0;
      min-width: 0;
      font-size: var(--font-size-xs);
    }

    .nav-item.active {
      border-left-color: transparent;
      border-bottom: 2px solid var(--accent-cyan);
    }

    .nav-label {
      font-size: 0.65rem;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
      max-width: 100%;
      text-align: center;
    }

    .nav-badge {
      position: absolute;
      top: 0;
      right: 2px;
      transform: scale(0.75);
    }

    .nav-item {
      position: relative;
    }
  }
</style>
