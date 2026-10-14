import Link from "next/link";
import type { ReactNode } from "react";
import type { DataScope, Party } from "@tideline/sdk";
import { methodLabel, shortAddress, spanLabel, utc, windowLabel } from "@/lib/format";

export function PageHeader({
  title,
  lede,
  children,
}: {
  title: string;
  lede?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <header className="flex flex-col gap-4 border-b border-line pb-6 md:flex-row md:items-end md:justify-between">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
        {lede ? <p className="mt-2 max-w-[65ch] text-sm leading-relaxed text-ink-2">{lede}</p> : null}
      </div>
      {children}
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
    <section className="flex min-w-0 flex-col gap-3">
      <div className="flex flex-wrap items-baseline justify-between gap-x-6 gap-y-1">
        <h2 className="text-sm font-semibold">{title}</h2>
        {note ? <p className="text-xs text-ink-3">{note}</p> : null}
      </div>
      {children}
    </section>
  );
}

export function Stat({ label, value, sub }: { label: string; value: ReactNode; sub?: ReactNode }) {
  return (
    <div className="flex flex-col gap-1 border-t border-line-strong pt-3">
      <dt className="text-xs text-ink-2">{label}</dt>
      <dd className="font-mono text-2xl tracking-tight">{value}</dd>
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
      className={`inline-flex items-center whitespace-nowrap rounded-sm border px-1.5 py-0.5 text-[11px] leading-none ${tones[tone]}`}
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

/** A flow counterparty: the entity when known, the address otherwise. */
export function PartyCell({ party }: { party: Party }) {
  const isAddress = /^0x[0-9a-f]{40}$/.test(party.address);
  const address = isAddress ? (
    <Link
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
    <nav aria-label="Time window" className="flex rounded-sm border border-line-strong text-xs">
      {[1, 24, 168].map((hours) => (
        <Link
          key={hours}
          href={`${basePath}?window=${hours}`}
          aria-current={hours === current ? "page" : undefined}
          className={`px-3 py-1.5 font-mono transition-colors active:translate-y-px ${
            hours === current ? "bg-ink text-page" : "text-ink-2 hover:bg-accent-wash hover:text-accent-ink"
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
    <p role="note" className="rounded-sm border border-line-strong px-4 py-3 text-sm leading-relaxed text-ink-2">
      Flows on file start at <span className="font-mono text-xs">{utc(scope.history_start)}</span>.
      The {windowLabel(hours)} window below holds {held} of data, not the full {windowLabel(hours)}.
    </p>
  );
}

export function Empty({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="rounded-sm border border-dashed border-line-strong px-5 py-8">
      <p className="text-sm font-medium">{title}</p>
      {children ? <div className="mt-1.5 max-w-[65ch] text-sm leading-relaxed text-ink-2">{children}</div> : null}
    </div>
  );
}

export function Code({ children }: { children: ReactNode }) {
  return (
    <pre className="overflow-x-auto rounded-sm border border-line bg-surface p-4 font-mono text-xs leading-relaxed text-ink-2">
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
        <thead className="border-b border-line-strong text-xs text-ink-2 [&_th]:px-3 [&_th]:py-2 [&_th]:font-medium [&_th:first-child]:pl-0 [&_th:last-child]:pr-0">
          {head}
        </thead>
        <tbody className="[&_td]:px-3 [&_td]:py-2.5 [&_td]:align-top [&_td:first-child]:pl-0 [&_td:last-child]:pr-0 [&_tr]:border-b [&_tr]:border-line">
          {children}
        </tbody>
      </table>
    </div>
  );
}
