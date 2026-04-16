/// Filtres structurés issus de la barre de recherche unifiée.
/// Structured filters from the unified search bar.
export interface SearchFilters {
  app?: string;
  dest?: string;
  src?: string;
  port?: number;
  sport?: number;
  dport?: number;
  proto?: string;
  verdict?: string;
  direction?: string;
  state?: string;
  user?: string;
  hostname?: string;
  freeText?: string;
}

/// Parse une requête de recherche structurée.
/// Syntaxe : app:firefox dest:google.com port:443 verdict:blocked
/// Les tokens sans ":" sont du texte libre (recherche globale).
///
/// Parse a structured search query.
/// Syntax: app:firefox dest:google.com port:443 verdict:blocked
/// Tokens without ":" are free-text (global search).
export function parseSearchQuery(query: string): SearchFilters {
  const filters: SearchFilters = {};
  const freeTextParts: string[] = [];

  for (const token of query.split(/\s+/).filter(Boolean)) {
    const colonIdx = token.indexOf(':');
    if (colonIdx > 0) {
      const key = token.slice(0, colonIdx).toLowerCase();
      const value = token.slice(colonIdx + 1);
      if (!value) continue;

      switch (key) {
        case 'app':
          filters.app = value;
          break;
        case 'dest':
          filters.dest = value;
          break;
        case 'src':
          filters.src = value;
          break;
        case 'port':
          filters.port = parseInt(value, 10) || undefined;
          break;
        case 'sport':
          filters.sport = parseInt(value, 10) || undefined;
          break;
        case 'dport':
          filters.dport = parseInt(value, 10) || undefined;
          break;
        case 'proto':
        case 'protocol':
          filters.proto = value.toLowerCase();
          break;
        case 'verdict':
          filters.verdict = value.toLowerCase();
          break;
        case 'direction':
        case 'dir':
          filters.direction = value.toLowerCase();
          break;
        case 'state':
          filters.state = value.toLowerCase();
          break;
        case 'user':
          filters.user = value;
          break;
        case 'hostname':
        case 'host':
          filters.hostname = value.toLowerCase();
          break;
        default:
          freeTextParts.push(token);
      }
    } else {
      freeTextParts.push(token);
    }
  }

  if (freeTextParts.length > 0) {
    filters.freeText = freeTextParts.join(' ');
  }
  return filters;
}

/// Vérifie si une IP correspond à un filtre (exact ou préfixe).
/// Check if an IP matches a filter (exact or prefix).
export function matchesIpFilter(ip: string, filter: string): boolean {
  if (!ip || !filter) return false;
  return ip.startsWith(filter) || ip === filter;
}

/// Extrait les filtres actifs sous forme de chips affichables.
/// Extract active filters as displayable chips.
export function getActiveChips(filters: SearchFilters): { key: string; value: string }[] {
  const chips: { key: string; value: string }[] = [];
  if (filters.app) chips.push({ key: 'app', value: filters.app });
  if (filters.dest) chips.push({ key: 'dest', value: filters.dest });
  if (filters.src) chips.push({ key: 'src', value: filters.src });
  if (filters.port) chips.push({ key: 'port', value: String(filters.port) });
  if (filters.sport) chips.push({ key: 'sport', value: String(filters.sport) });
  if (filters.dport) chips.push({ key: 'dport', value: String(filters.dport) });
  if (filters.proto) chips.push({ key: 'proto', value: filters.proto });
  if (filters.verdict) chips.push({ key: 'verdict', value: filters.verdict });
  if (filters.direction) chips.push({ key: 'direction', value: filters.direction });
  if (filters.state) chips.push({ key: 'state', value: filters.state });
  if (filters.user) chips.push({ key: 'user', value: filters.user });
  if (filters.hostname) chips.push({ key: 'hostname', value: filters.hostname });
  if (filters.freeText) chips.push({ key: 'texte', value: filters.freeText });
  return chips;
}
