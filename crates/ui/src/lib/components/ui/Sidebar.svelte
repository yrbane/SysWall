<script lang="ts">
  import { page } from '$app/stores';

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

  // Sépare paramètres des items principaux / Separate settings from main items
  const mainItems = $derived(items.filter(i => i.route !== '/settings'));
  const settingsItem = $derived(items.find(i => i.route === '/settings'));
</script>

<nav class="sidebar" aria-label="Navigation principale">
  <!-- Logo -->
  <div class="sidebar-logo">
    <div class="logo-icon">🛡️</div>
    <span class="logo-text">SysWall</span>
  </div>

  <!-- Navigation principale / Main navigation -->
  <div class="nav-items">
    {#each mainItems as item}
      <a
        href={item.route}
        class="nav-item"
        class:active={currentPath === item.route || currentPath.startsWith(item.route + '/')}
        aria-current={currentPath === item.route ? 'page' : undefined}
      >
        <span class="nav-icon">{item.icon}</span>
        <span class="nav-label">{item.label}</span>
        {#if item.badge && item.badge > 0}
          <span class="nav-badge" class:pulsing={item.pulsing} class:orange={item.pulsing}>
            {item.badge}
          </span>
        {/if}
      </a>
    {/each}
  </div>

  <!-- Paramètres en bas / Settings at bottom -->
  {#if settingsItem}
    <div class="nav-footer">
      <a
        href={settingsItem.route}
        class="nav-item nav-item-settings"
        class:active={currentPath === settingsItem.route}
      >
        <span class="nav-icon">{settingsItem.icon}</span>
        <span class="nav-label">{settingsItem.label}</span>
      </a>
    </div>
  {/if}
</nav>

<style>
  .sidebar {
    width: var(--sidebar-width);
    height: 100vh;
    background: var(--bg-sidebar);
    backdrop-filter: blur(var(--glass-blur));
    border-right: 1px solid var(--border-primary);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    position: fixed;
    top: 0;
    left: 0;
    z-index: 50;
  }

  /* --- Logo --- */
  .sidebar-logo {
    padding: 12px 14px 16px;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .logo-icon {
    width: 26px;
    height: 26px;
    border-radius: 6px;
    background: linear-gradient(135deg, var(--accent-blue), var(--accent-purple));
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 14px;
  }

  .logo-text {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    letter-spacing: -0.2px;
  }

  /* --- Navigation items --- */
  .nav-items {
    flex: 1;
    padding: 0 6px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 6px 10px;
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    text-decoration: none;
    font-size: 12px;
    font-weight: 400;
    transition: background var(--transition-fast);
  }

  .nav-item:hover {
    background: var(--bg-hover);
  }

  .nav-item.active {
    background: var(--accent-cyan-15);
    color: var(--accent-blue);
    font-weight: 500;
  }

  .nav-icon {
    width: 18px;
    font-size: 14px;
    text-align: center;
    flex-shrink: 0;
  }

  .nav-label {
    flex: 1;
  }

  .nav-badge {
    margin-left: auto;
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-secondary);
    font-size: 9px;
    padding: 1px 6px;
    border-radius: 10px;
    font-weight: 500;
    font-family: var(--font-mono);
  }

  .nav-badge.orange {
    background: var(--accent-orange);
    color: #000;
    font-weight: 600;
  }

  .nav-badge.pulsing {
    animation: badgePulse 2s infinite;
  }

  @keyframes badgePulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.6; }
  }

  /* --- Footer / Settings --- */
  .nav-footer {
    padding: 6px;
    border-top: 1px solid var(--border-primary);
  }

  .nav-item-settings {
    color: var(--text-secondary);
  }

  /* --- Tablette : icônes seules / Tablet: icons only --- */
  @media (min-width: 640px) and (max-width: 1024px) {
    .sidebar { width: 56px; }
    .sidebar-logo { padding: 12px 0; justify-content: center; }
    .logo-text { display: none; }
    .nav-items { padding: 0 4px; }
    .nav-item { justify-content: center; padding: 8px; }
    .nav-label { display: none; }
    .nav-badge {
      position: absolute;
      top: 0;
      right: 0;
      transform: scale(0.8);
    }
    .nav-item { position: relative; }
    .nav-footer { padding: 4px; }
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
      backdrop-filter: blur(var(--glass-blur));
    }
    .sidebar-logo { display: none; }
    .nav-footer { display: none; }
    .nav-items {
      flex-direction: row;
      padding: 0;
      overflow-x: auto;
      gap: 0;
    }
    .nav-item {
      flex: 1;
      flex-direction: column;
      align-items: center;
      gap: 2px;
      padding: 12px 8px;
      min-width: 44px;
      min-height: 44px;
      box-sizing: border-box;
      font-size: 9px;
      border-radius: 0;
    }
    .nav-item.active {
      border-bottom: 2px solid var(--accent-blue);
      background: none;
    }
  }
</style>
