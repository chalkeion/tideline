import Link from "next/link";
import { Empty, PageHeader, ScopeNote, Section, Stat, Table, WindowTabs } from "@/components/ui";
import { api, parseWindow } from "@/lib/api";
import { count, methodLabel, ms, pct, usd, windowLabel } from "@/lib/format";

export const dynamic = "force-dynamic";
export const metadata = { title: "Coverage" };

const VISIBILITY: Record<string, { label: string; meaning: string }> = {
  public: { label: "Public", meaning: "Amount and both counterparties visible on-chain." },
  shielded: {
    label: "Amount shielded",
    meaning: "Counterparties visible, amount encrypted. Attributed and counted, excluded from USD totals.",
  },
  disclosed: {
    label: "Disclosed",
    meaning: "Shielded on-chain, amount revealed to Tideline by a view-key holder.",
  },
};

export default async function CoveragePage({
  searchParams,
}: {
  searchParams: Promise<{ window?: string }>;
}) {
  const hours = parseWindow((await searchParams).window);
  const c = await api.coverage(hours);
  const label = windowLabel(hours);

  return (
    <>
      <PageHeader
        title="Attribution coverage"
        lede="One checkable number: how much Arc stablecoin flow Tideline ties to a known entity, and how soon after settlement. Recomputed from stored flows on every load."
      >
        <WindowTabs basePath="/coverage" current={hours} />
      </PageHeader>

      <ScopeNote scope={c.scope} hours={hours} />

      <section className="rounded-sm border border-line-strong bg-surface p-6">
        {c.claim_volume_pct === null ? (
          <p className="max-w-[65ch] text-lg leading-snug">
            No live flows in the last {label}, so there is nothing to claim yet. Latency is only
            measured on blocks produced while the ingestor follows the chain head. Backfilled
            history is excluded.
          </p>
        ) : (
          <p className="max-w-[65ch] text-lg leading-snug">
            <span className="font-mono text-4xl font-semibold tracking-tight">{pct(c.claim_volume_pct)}</span>{" "}
            of live volume in the last {label} was attributed to a known entity within{" "}
            {c.claim_within_seconds} seconds of settlement, across {count(c.live_flows)} live flows.
          </p>
        )}
      </section>

      <dl className="grid grid-cols-2 gap-6 md:grid-cols-4">
        <Stat
          label="Volume attributed"
          value={pct(c.volume_coverage_pct)}
          sub={`${usd(c.volume_usd_attributed)} of ${usd(c.volume_usd_total)}`}
        />
        <Stat
          label="Flows attributed"
          value={pct(c.flow_coverage_pct)}
          sub={`${count(c.flows_attributed)} of ${count(c.flows_total)}, ${count(c.flows_fully_attributed)} on every side`}
        />
        <Stat
          label="Proprietary coverage"
          value={pct(c.institutional_volume_coverage_pct)}
          sub="volume, excluding protocol-registry labels"
        />
        <Stat label="Latency p50 / p95" value={`${ms(c.latency_p50_ms)} / ${ms(c.latency_p95_ms)}`} sub="settlement to queryable" />
      </dl>

      <div className="grid gap-10 lg:grid-cols-2">
        <Section title="How a flow counts as attributed">
          <ul className="flex max-w-[65ch] flex-col gap-2 text-sm leading-relaxed text-ink-2">
            <li>
              A flow is attributed when at least one real counterparty resolves to an entity in
              the registry. The zero address (mint and burn) is never a counterparty.
            </li>
            <li>
              Protocol-registry labels cover Arc&apos;s own contracts. Proprietary coverage
              leaves them out and counts only self-attested, curated and heuristic attributions.
            </li>
            <li>
              Latency is the time from the block timestamp to the flow being stored and
              queryable, for flows ingested live. Block timestamps have one-second resolution.
            </li>
          </ul>
        </Section>
        <Section title="Registry by attribution method">
          <Table
            head={
              <tr>
                <th>Method</th>
                <th className="text-right">Addresses</th>
              </tr>
            }
          >
            {c.methods.map((m) => (
              <tr key={m.method}>
                <td>{methodLabel(m.method)}</td>
                <td className="text-right font-mono">{count(m.addresses)}</td>
              </tr>
            ))}
          </Table>
        </Section>
      </div>

      <Section
        title="Visibility under Arc's privacy model"
        note="Arc confidential transfers hide amounts, not addresses"
      >
        <p className="max-w-[65ch] text-sm leading-relaxed text-ink-2">
          Attribution works on counterparties, so it survives amount shielding. Volume figures
          state exactly what they are built from: shielded flows are counted here and never
          silently treated as zero.
        </p>
        {c.scope.shielding_live ? null : (
          <p className="max-w-[65ch] text-sm leading-relaxed text-ink-2">
            Arc has not shipped confidential transfers yet, so every flow on file is public.
            Shielded and disclosed rows appear here once Arc publishes the event and Tideline
            starts ingesting it.
          </p>
        )}
        {c.visibility.length === 0 ? (
          <Empty title="No flows in this window" />
        ) : (
          <Table
            head={
              <tr>
                <th>Visibility</th>
                <th>What Tideline sees</th>
                <th className="text-right">Flows</th>
                <th className="text-right">Volume</th>
              </tr>
            }
          >
            {c.visibility.map((v) => (
              <tr key={v.visibility}>
                <td className="font-medium">{VISIBILITY[v.visibility]?.label ?? v.visibility}</td>
                <td className="text-ink-2">{VISIBILITY[v.visibility]?.meaning}</td>
                <td className="text-right font-mono">{count(v.flows)}</td>
                <td className="text-right font-mono">
                  {v.visibility === "shielded" ? "not observable" : usd(v.volume_usd)}
                </td>
              </tr>
            ))}
          </Table>
        )}
      </Section>

      <Section
        title="Curation worklist"
        note="Unattributed addresses by volume. Labelling one adds its share to coverage."
      >
        {c.top_unattributed.length === 0 ? (
          <Empty title="Nothing to label in this window" />
        ) : (
          <Table
            head={
              <tr>
                <th>Address</th>
                <th className="text-right">Flows</th>
                <th className="text-right">Volume touched</th>
                <th className="text-right">Share of window volume</th>
              </tr>
            }
          >
            {c.top_unattributed.map((row) => (
              <tr key={row.address}>
                <td>
                  {/^0x[0-9a-f]{40}$/.test(row.address) ? (
                    <Link href={`/address/${row.address}`} className="font-mono text-xs hover:text-accent-ink hover:underline">
                      {row.address}
                    </Link>
                  ) : (
                    <span className="break-all font-mono text-xs">{row.address}</span>
                  )}
                </td>
                <td className="text-right font-mono">{count(row.flows)}</td>
                <td className="text-right font-mono">{usd(row.volume_usd)}</td>
                <td className="text-right font-mono">{pct(row.volume_share_pct)}</td>
              </tr>
            ))}
          </Table>
        )}
      </Section>
    </>
  );
}
