<script lang="ts">
  import { page } from '$app/stores';
  import SyswallLogo from '$lib/components/branding/SyswallLogo.svelte';
  import { LayoutDashboard, Network, Shield, BrainCircuit, Ban, ClipboardList, Settings } from 'lucide-svelte';
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  type IconComponent = any;

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

  // Table de correspondance route → composant Lucide / Route to Lucide component map
  const iconMap: Record<string, IconComponent> = {
    '/dashboard': LayoutDashboard,
    '/connections': Network,
    '/rules': Shield,
    '/learning': BrainCircuit,
    '/blocklists': Ban,
    '/audit': ClipboardList,
    '/settings': Settings,
  };

  function getIcon(route: string): IconComponent {
    return iconMap[route] ?? Shield;
  }
</script>

<nav class="sidebar" aria-label="Navigation principale">
  <!-- Logo -->
  <div class="sidebar-logo">
    <SyswallLogo variant="full" size={20} />
  </div>

  <!-- Navigation principale / Main navigation -->
  <div class="nav-items">
    {#each mainItems as item}
      {@const IconComponent = getIcon(item.route)}
      <a
        href={item.route}
        class="nav-item"
        class:active={currentPath === item.route || currentPath.startsWith(item.route + '/')}
        aria-current={currentPath === item.route ? 'page' : undefined}
      >
        <span class="nav-icon">
          <IconComponent size={16} strokeWidth={1.75} />
        </span>
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
    {@const SettingsIcon = getIcon(settingsItem.route)}
    <div class="nav-footer">
      <a
        href={settingsItem.route}
        class="nav-item nav-item-settings"
        class:active={currentPath === settingsItem.route}
      >
        <span class="nav-icon">
          <SettingsIcon size={16} strokeWidth={1.75} />
        </span>
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

  .sidebar-logo :global(svg) {
    /* Logotype SysWall — hérité de currentColor pour le texte */
    color: var(--text-primary);
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
    height: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  /* Héritage de couleur pour les icônes Lucide / Lucide icons inherit color */
  .nav-icon :global(svg) {
    color: inherit;
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
