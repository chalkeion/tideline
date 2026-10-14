const KIND_LABELS: Record<string, string> = {
  transfer: "Transfer",
  mint: "Issuance",
  burn: "Redemption",
  cctp_outbound: "CCTP out",
  cctp_inbound: "CCTP in",
  gateway_deposit: "Gateway deposit",
  gateway_outbound: "Gateway out",
  gateway_inbound: "Gateway in",
  usyc_subscription: "USYC subscription",
  usyc_redemption: "USYC redemption",
  fx_settlement: "StableFX",
};

export const KINDS = Object.keys(KIND_LABELS);
/** Listed assets in fixed chart order: each keeps its series colour whatever else is shown. */
export const SERIES_ASSETS = ["USDC", "EURC", "USYC", "cirBTC", "WETH"] as const;

const PRICE_LABELS: Record<string, string> = {
  usd: "1.00 USD",
  eur: "ECB euro reference rate",
  usyc: "USYC fund price report",
  btc: "Bitcoin spot price",
  eth: "Ether spot price",
};

/** How a token is valued, or that it is not. */
export function priceLabel(source: string | null): string {
  return source ? (PRICE_LABELS[source] ?? source) : "Unpriced";
}

export function standardLabel(standard: string): string {
  return standard === "erc721" ? "NFT (ERC-721)" : "ERC-20";
}

/** A token amount for tables: compact above a million, otherwise up to four decimals. */
export function tokenAmount(units: string | null): string {
  if (units === null) return "Shielded";
  const value = Number(units);
  if (!Number.isFinite(value)) return units;
  if (Math.abs(value) >= 1e6) {
    return new Intl.NumberFormat("en-US", { notation: "compact", maximumFractionDigits: 2 }).format(value);
  }
  if (value !== 0 && Math.abs(value) < 0.0001) {
    return value.toLocaleString("en-US", { maximumSignificantDigits: 3 });
  }
  return value.toLocaleString("en-US", { maximumFractionDigits: 4 });
}

/** An NFT id short enough for a table cell. */
export function tokenId(id: string): string {
  return id.length > 12 ? `${id.slice(0, 6)}…${id.slice(-4)}` : id;
}

export function kindLabel(kind: string): string {
  return KIND_LABELS[kind] ?? kind;
}

const METHOD_LABELS: Record<string, string> = {
  protocol_registry: "Protocol registry",
  self_attested: "Self-attested",
  curated: "Curated",
  heuristic: "Heuristic",
};

export function methodLabel(method: string): string {
  return METHOD_LABELS[method] ?? method;
}

const CATEGORY_LABELS: Record<string, string> = {
  issuer: "Issuer",
  protocol: "Protocol",
  asset_manager: "Asset manager",
  bank: "Bank",
  payments: "Payments",
  market_infra: "Market infrastructure",
  trading: "Trading",
  infrastructure: "Infrastructure",
};

export function categoryLabel(category: string): string {
  return CATEGORY_LABELS[category] ?? category;
}

export function usd(value: number | null | undefined): string {
  if (value === null || value === undefined) return "Shielded";
  const abs = Math.abs(value);
  if (abs >= 1e9) return `$${(value / 1e9).toFixed(2)}B`;
  if (abs >= 1e6) return `$${(value / 1e6).toFixed(2)}M`;
  if (abs >= 1e4) return `$${(value / 1e3).toFixed(1)}K`;
  return `$${value.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}

export function count(value: number): string {
  return value.toLocaleString("en-US");
}

export function pct(value: number | null | undefined): string {
  return value === null || value === undefined ? "n/a" : `${value.toFixed(1)}%`;
}

export function ms(value: number | null | undefined): string {
  if (value === null || value === undefined) return "n/a";
  return value >= 1000 ? `${(value / 1000).toFixed(2)} s` : `${Math.round(value)} ms`;
}

export function shortAddress(address: string): string {
  return address.length > 14 ? `${address.slice(0, 8)}…${address.slice(-6)}` : address;
}

/** Absolute UTC, so a server-rendered time never goes stale or mismatches on hydration. */
export function utc(iso: string): string {
  const d = new Date(iso);
  const date = d.toISOString();
  return `${date.slice(0, 10)} ${date.slice(11, 19)} UTC`;
}

export function utcTime(iso: string): string {
  return new Date(iso).toISOString().slice(11, 19);
}

/** A duration in the largest unit that keeps it honest: "21 minutes", "5 hours", "3 days". */
export function spanLabel(milliseconds: number): string {
  const minutes = Math.max(1, Math.floor(milliseconds / 60_000));
  const unit = (value: number, name: string) => `${value} ${name}${value === 1 ? "" : "s"}`;
  if (minutes < 120) return unit(minutes, "minute");
  const hours = Math.floor(minutes / 60);
  return hours < 48 ? unit(hours, "hour") : unit(Math.floor(hours / 24), "day");
}

/** A reference rate with its provenance: "1.1186 USD (ECB euro reference rate, 2026-10-08)". */
export function rateLabel(rate: number, source: string, asOf: string | null): string {
  return `${rate.toFixed(4)} USD (${source}${asOf ? `, ${asOf}` : ""})`;
}

export function windowLabel(hours: number): string {
  if (hours < 24) return `${hours}h`;
  return hours === 24 ? "24h" : `${hours / 24}d`;
}

/** Transaction page on the indexed network's explorer (`NetworkInfo.explorer_url`). */
export function explorerTx(explorerUrl: string, hash: string): string {
  return `${explorerUrl}/tx/${hash}`;
}

export function isEvmAddress(value: string): boolean {
  return /^0x[0-9a-fA-F]{40}$/.test(value);
}
