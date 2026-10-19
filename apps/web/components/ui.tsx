import Link from "next/link";
import type { ReactNode } from "react";
import type { DataScope, Party } from "@tideline/sdk";
import { methodLabel, shortAddress, spanLabel, utc, windowLabel } from "@/lib/format";

/**
 * A pill link whose arrow sits in its own circle and nudges up and to the right on hover.
 */
export function ArrowLink({ href, children }: { href: string; children: ReactNode }) {
  return (
    <Link
      prefetch={false}
      href={href}
      className="group inline-flex items-center gap-3 rounded-full border border-line-strong bg-ink/[0.04] py-1.5 pl-4 pr-1.5 text-sm transition-[background-color,transform] duration-500 ease-tide hover:bg-ink/[0.07] active:scale-[0.98]"
    >
      {children}
      <span className="flex size-7 items-center justify-center rounded-full bg-accent text-page transition-transform duration-500 ease-tide group-hover:-translate-y-px group-hover:translate-x-0.5 group-hover:scale-105">
        <svg viewBox="0 0 16 16" className="size-3.5" fill="none" aria-hidden="true">
          <path d="M5 11 11 5M6 5h5v5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </span>
    </Link>
  );
}

/** A small pill label above a heading. */
export function Eyebrow({ children }: { children: ReactNode }) {
  return (
    <span className="inline-flex w-fit items-center gap-2 self-start rounded-full border border-line-strong bg-shell px-3 py-1 text-[10px] font-medium uppercase tracking-[0.2em] text-ink-2">
      {children}
    </span>
  );
}

export function PageHeader({
  title,
  lede,
  eyebrow,
  children,
}: {
  title: string;
  lede?: ReactNode;
  eyebrow?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <header className="flex flex-col gap-6 md:flex-row md:items-end md:justify-between">
      <div className="flex min-w-0 flex-col items-start gap-4">
        {eyebrow ? <Eyebrow>{eyebrow}</Eyebrow> : null}
        <h1 className="max-w-[24ch] text-[2rem] font-semibold leading-[1.05] tracking-[-0.035em] text-balance md:text-[2.75rem]">
          {title}
        </h1>
        {lede ? <p className="max-w-[62ch] text-[15px] leading-relaxed text-ink-2 text-pretty">{lede}</p> : null}
      </div>
      {children ? <div className="shrink-0">{children}</div> : null}
    </header>
  );
}

export function Section({
  title,
  note,
  children,
}: {
  title: string;
  note?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="bezel min-w-0">
      <div className="bezel-core flex min-w-0 flex-col gap-4 p-5 md:p-6">
        <div className="flex flex-wrap items-baseline justify-between gap-x-6 gap-y-1">
          <h2 className="text-[13px] font-semibold tracking-[-0.01em]">{title}</h2>
          {note ? <p className="text-xs text-ink-3">{note}</p> : null}
        </div>
        {children}
      </div>
    </section>
  );
}

export function Stat({ label, value, sub }: { label: string; value: ReactNode; sub?: ReactNode }) {
  return (
    <div className="bezel-solo flex flex-col justify-between gap-3 px-5 py-4">
      <div className="flex flex-col gap-2">
        <dt className="text-xs text-ink-2">{label}</dt>
        <dd className="text-[1.65rem] font-medium leading-none tracking-[-0.035em] tabular-nums">{value}</dd>
      </div>
      {sub ? <p className="text-xs text-ink-3">{sub}</p> : null}
    </div>
  );
}

export function Tag({
  children,
  tone = "neutral",
}: {
  children: ReactNode;
  tone?: "neutral" | "accent" | "muted";
}) {
  const tones = {
    neutral: "border-line-strong text-ink-2",
    accent: "border-accent bg-accent-wash text-accent-ink",
    muted: "border-line text-ink-3",
  };
  return (
    <span
      className={`inline-flex items-center whitespace-nowrap rounded-full border px-2 py-0.5 text-[11px] leading-[1.4] ${tones[tone]}`}
    >
      {children}
    </span>
  );
}

const ATTRIBUTION: Record<string, { label: string; tone: "accent" | "neutral" | "muted" }> = {
  full: { label: "Attributed", tone: "accent" },
  partial: { label: "Partial", tone: "neutral" },
  none: { label: "Unattributed", tone: "muted" },
};

export function AttributionTag({ attribution }: { attribution: string }) {
  const a = ATTRIBUTION[attribution] ?? ATTRIBUTION.none!;
  return <Tag tone={a.tone}>{a.label}</Tag>;
}

/**
 * A token by symbol, linked to its asset page. An unlisted token is marked as such: its
 * symbol is whatever its deployer chose, and may copy a listed token's.
 */
export function AssetName({
  symbol,
  address,
  listed,
}: {
  symbol: string;
  address: string;
  listed: boolean;
}) {
  return (
    <Link
      prefetch={false}
      href={`/assets/${address}`}
      title={listed ? `${symbol} ${address}` : `Unlisted token ${address}`}
      className="underline-offset-2 hover:text-accent-ink hover:underline"
    >
      {symbol}
      {listed ? null : <span className="text-ink-3"> (unlisted)</span>}
    </Link>
  );
}

/** A flow counterparty: the entity when known, the address otherwise. */
export function PartyCell({ party }: { party: Party }) {
  const isAddress = /^0x[0-9a-f]{40}$/.test(party.address);
  const address = isAddress ? (
    <Link
      prefetch={false}
      href={`/address/${party.address}`}
      className="font-mono text-xs text-ink-2 underline-offset-2 hover:text-accent-ink hover:underline"
      title={party.address}
    >
      {shortAddress(party.address)}
    </Link>
  ) : (
    <span className="font-mono text-xs text-ink-2" title={party.address}>
      {shortAddress(party.address)}
    </span>
  );
  if (!party.entity) return address;
  return (
    <span className="flex flex-col gap-0.5">
      <Link
        prefetch={false}
        href={`/entities/${party.entity.slug}`}
        className="text-sm font-medium underline-offset-2 hover:text-accent-ink hover:underline"
        title={party.method ? `${methodLabel(party.method)}, confidence ${party.confidence?.toFixed(2)}` : undefined}
      >
        {party.entity.name}
      </Link>
      {address}
    </span>
  );
}

export function WindowTabs({ basePath, current }: { basePath: string; current: number }) {
  return (
    <nav aria-label="Time window" className="flex gap-0.5 rounded-full border border-line-strong bg-shell p-1 text-xs">
      {[1, 24, 168].map((hours) => (
        <Link
          prefetch={false}
          key={hours}
          href={`${basePath}?window=${hours}`}
          aria-current={hours === current ? "page" : undefined}
          className={`rounded-full px-3.5 py-1.5 font-mono transition-[color,background-color] duration-500 ease-tide active:scale-[0.97] ${
            hours === current
              ? "bg-ink text-page shadow-[0_6px_16px_-8px_var(--shadow)]"
              : "text-ink-2 hover:bg-ink/[0.05] hover:text-ink"
          }`}
        >
          {windowLabel(hours)}
        </Link>
      ))}
    </nav>
  );
}

/**
 * Says so when a window reaches back further than the data does, so a "24h" total is
 * never read as a full day when only part of it is on file.
 */
export function ScopeNote({ scope, hours }: { scope: DataScope; hours: number }) {
  if (!scope.history_start || scope.history_seconds === null || scope.window_complete) return null;
  const held = spanLabel(scope.history_seconds * 1000);
  return (
    <p role="note" className="rounded-2xl border border-line-strong bg-shell px-5 py-3.5 text-sm leading-relaxed text-ink-2">
      Flows on file start at <span className="font-mono text-xs">{utc(scope.history_start)}</span>.
      The {windowLabel(hours)} window below holds {held} of data, not the full {windowLabel(hours)}.
    </p>
  );
}

export function Empty({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="rounded-2xl border border-dashed border-line-strong px-5 py-8">
      <p className="text-sm font-medium">{title}</p>
      {children ? <div className="mt-1.5 max-w-[65ch] text-sm leading-relaxed text-ink-2">{children}</div> : null}
    </div>
  );
}

export function Code({ children }: { children: ReactNode }) {
  return (
    <pre className="overflow-x-auto rounded-xl border border-line bg-shell p-4 font-mono text-xs leading-relaxed text-ink-2">
      {children}
    </pre>
  );
}

/** Table shell: header row and row dividers only, no outer card. */
export function Table({
  head,
  children,
  wide = false,
}: {
  head: ReactNode;
  children: ReactNode;
  /** Many-column tables keep a readable minimum width and scroll sideways on small screens. */
  wide?: boolean;
}) {
  return (
    <div className="overflow-x-auto">
      <table className={`w-full border-collapse text-left text-sm ${wide ? "min-w-[760px]" : ""}`}>
        <thead className="border-b border-line-strong text-[11px] uppercase tracking-[0.12em] text-ink-3 [&_th]:px-3 [&_th]:py-2.5 [&_th]:font-medium [&_th:first-child]:pl-0 [&_th:last-child]:pr-0">
          {head}
        </thead>
        <tbody className="[&_td]:px-3 [&_td]:py-3 [&_td]:align-top [&_td:first-child]:pl-0 [&_td:last-child]:pr-0 [&_tr]:border-b [&_tr]:border-line [&_tr:last-child]:border-b-0 [&_tr]:transition-colors [&_tr]:duration-300 [&_tr:hover]:bg-ink/[0.02]">
          {children}
        </tbody>
      </table>
    </div>
  );
}
