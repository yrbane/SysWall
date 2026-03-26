<script lang="ts">
  import { fr } from '$lib/i18n/fr';
  import Input from '$lib/components/ui/Input.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import type { RuleMessage, RuleCriteria } from '$lib/types';

  interface Props {
    /** If provided, we are editing an existing rule */
    editRule?: RuleMessage | null;
    onsave: (data: { name: string; priority: number; effect: string; criteria: RuleCriteria; source: string }) => void;
    oncancel: () => void;
    saving?: boolean;
  }

  let { editRule = null, onsave, oncancel, saving = false }: Props = $props();

  // Form state
  let name = $state(editRule?.name || '');
  let priorityStr = $state(String(editRule?.priority ?? 100));
  let effect = $state(editRule?.effect || 'allow');
  let source = $state(editRule?.source || 'manual');

  // Parse existing criteria from JSON
  const existingCriteria: RuleCriteria = editRule?.criteria_json
    ? (() => { try { return JSON.parse(editRule.criteria_json); } catch { return {}; } })()
    : {};

  // Criteria sections - toggled on/off
  let showApp = $state(!!existingCriteria.application);
  let showUser = $state(!!existingCriteria.user);
  let showRemoteIp = $state(!!existingCriteria.remote_ip);
  let showRemotePort = $state(!!existingCriteria.remote_port);
  let showProtocol = $state(!!existingCriteria.protocol);
  let showDirection = $state(!!existingCriteria.direction);

  // Criteria values
  let appName = $state(existingCriteria.application?.name || '');
  let appPath = $state(existingCriteria.application?.path || '');
  let userName = $state(existingCriteria.user || '');
  let remoteIpExact = $state(existingCriteria.remote_ip?.exact || '');
  let remoteIpCidr = $state(existingCriteria.remote_ip?.cidr || '');
  let remotePortExact = $state(existingCriteria.remote_port?.exact?.toString() || '');
  let remotePortRangeStart = $state(existingCriteria.remote_port?.range?.[0]?.toString() || '');
  let remotePortRangeEnd = $state(existingCriteria.remote_port?.range?.[1]?.toString() || '');
  let protocolValue = $state(existingCriteria.protocol || 'tcp');
  let directionValue = $state(existingCriteria.direction || 'outbound');

  function buildCriteria(): RuleCriteria {
    const criteria: RuleCriteria = {};
    if (showApp && (appName || appPath)) {
      criteria.application = {};
      if (appName) criteria.application.name = appName;
      if (appPath) criteria.application.path = appPath;
    }
    if (showUser && userName) {
      criteria.user = userName;
    }
    if (showRemoteIp) {
      criteria.remote_ip = {};
      if (remoteIpExact) criteria.remote_ip.exact = remoteIpExact;
      if (remoteIpCidr) criteria.remote_ip.cidr = remoteIpCidr;
    }
    if (showRemotePort) {
      if (remotePortExact) {
        criteria.remote_port = { exact: parseInt(remotePortExact, 10) };
      } else if (remotePortRangeStart && remotePortRangeEnd) {
        criteria.remote_port = {
          range: [parseInt(remotePortRangeStart, 10), parseInt(remotePortRangeEnd, 10)],
        };
      }
    }
    if (showProtocol) {
      criteria.protocol = protocolValue;
    }
    if (showDirection) {
      criteria.direction = directionValue;
    }
    return criteria;
  }

  function handleSubmit() {
    onsave({
      name,
      priority: parseInt(priorityStr, 10) || 100,
      effect,
      criteria: buildCriteria(),
      source,
    });
  }
</script>

<form class="rule-form" onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
  <!-- Basic fields -->
  <div class="form-section">
    <Input label={fr.rules_name} bind:value={name} placeholder="e.g. Allow Firefox HTTPS" />

    <div class="form-row">
      <Input label={fr.rules_priority} type="number" bind:value={priorityStr} />

      <div class="select-group">
        <label class="select-label">{fr.rules_effect}</label>
        <select class="form-select" bind:value={effect}>
          <option value="allow">{fr.rules_allow}</option>
          <option value="block">{fr.rules_block}</option>
          <option value="ask">{fr.rules_ask}</option>
          <option value="observe">{fr.rules_observe}</option>
        </select>
      </div>
    </div>
  </div>

  <!-- Criteria builder -->
  <div class="form-section">
    <h3 class="section-title">{fr.rules_criteria}</h3>

    <!-- Application criteria -->
    <div class="criteria-toggle">
      <label class="toggle-label">
        <input type="checkbox" bind:checked={showApp} />
        {fr.criteria_application}
      </label>
    </div>
    {#if showApp}
      <div class="criteria-fields">
        <Input label={fr.rules_app_name} bind:value={appName} placeholder="firefox" />
        <Input label={fr.rules_app_path} bind:value={appPath} placeholder="/usr/bin/firefox" />
      </div>
    {/if}

    <!-- User criteria -->
    <div class="criteria-toggle">
      <label class="toggle-label">
        <input type="checkbox" bind:checked={showUser} />
        {fr.criteria_user}
      </label>
    </div>
    {#if showUser}
      <div class="criteria-fields">
        <Input label={fr.criteria_user} bind:value={userName} placeholder="root" />
      </div>
    {/if}

    <!-- Remote IP criteria -->
    <div class="criteria-toggle">
      <label class="toggle-label">
        <input type="checkbox" bind:checked={showRemoteIp} />
        {fr.criteria_remote_ip}
      </label>
    </div>
    {#if showRemoteIp}
      <div class="criteria-fields">
        <Input label={fr.rules_ip_exact} bind:value={remoteIpExact} placeholder="192.168.1.1" />
        <Input label={fr.rules_ip_cidr} bind:value={remoteIpCidr} placeholder="10.0.0.0/8" />
      </div>
    {/if}

    <!-- Remote port criteria -->
    <div class="criteria-toggle">
      <label class="toggle-label">
        <input type="checkbox" bind:checked={showRemotePort} />
        {fr.criteria_remote_port}
      </label>
    </div>
    {#if showRemotePort}
      <div class="criteria-fields">
        <Input label={fr.rules_port_exact} type="number" bind:value={remotePortExact} placeholder="443" />
        <div class="form-row">
          <Input label={fr.rules_port_range_start} type="number" bind:value={remotePortRangeStart} placeholder="1024" />
          <Input label={fr.rules_port_range_end} type="number" bind:value={remotePortRangeEnd} placeholder="65535" />
        </div>
      </div>
    {/if}

    <!-- Protocol criteria -->
    <div class="criteria-toggle">
      <label class="toggle-label">
        <input type="checkbox" bind:checked={showProtocol} />
        {fr.criteria_protocol}
      </label>
    </div>
    {#if showProtocol}
      <div class="criteria-fields">
        <div class="select-group">
          <select class="form-select" bind:value={protocolValue}>
            <option value="tcp">TCP</option>
            <option value="udp">UDP</option>
            <option value="icmp">ICMP</option>
          </select>
        </div>
      </div>
    {/if}

    <!-- Direction criteria -->
    <div class="criteria-toggle">
      <label class="toggle-label">
        <input type="checkbox" bind:checked={showDirection} />
        {fr.criteria_direction}
      </label>
    </div>
    {#if showDirection}
      <div class="criteria-fields">
        <div class="select-group">
          <select class="form-select" bind:value={directionValue}>
            <option value="inbound">{fr.conn_inbound}</option>
            <option value="outbound">{fr.conn_outbound}</option>
          </select>
        </div>
      </div>
    {/if}
  </div>

  <!-- Form actions -->
  <div class="form-actions">
    <Button variant="ghost" onclick={oncancel}>
      {fr.rules_cancel}
    </Button>
    <Button variant="primary" type="submit" loading={saving} disabled={!name}>
      {editRule ? fr.rules_save : fr.rules_create}
    </Button>
  </div>
</form>

<style>
  .rule-form {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
  }

  .form-section {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .section-title {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-semibold);
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .form-row {
    display: flex;
    gap: var(--space-4);
  }

  .form-row > :global(*) {
    flex: 1;
  }

  .select-group {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .select-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .form-select {
    background: var(--bg-tertiary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    padding: var(--space-2) var(--space-3);
    color: var(--text-primary);
    font-family: var(--font-sans);
    font-size: var(--font-size-sm);
    cursor: pointer;
    outline: none;
    width: 100%;
  }

  .form-select:focus {
    border-color: var(--accent-cyan);
  }

  .form-select option {
    background: var(--bg-secondary);
  }

  .criteria-toggle {
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--border-subtle);
  }

  .toggle-label {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--font-size-sm);
    color: var(--text-primary);
    cursor: pointer;
    font-weight: var(--font-weight-medium);
  }

  .toggle-label input[type='checkbox'] {
    accent-color: var(--accent-cyan);
    width: 16px;
    height: 16px;
  }

  .criteria-fields {
    padding: var(--space-3) 0 var(--space-3) var(--space-6);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    animation: fadeIn 150ms ease;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  .form-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-3);
    padding-top: var(--space-4);
    border-top: 1px solid var(--border-primary);
  }
</style>
