import Link from "next/link";
import { FlowTable } from "@/components/flow-table";
import { LiveFeed } from "@/components/live-feed";
import { AssetName, Empty, PageHeader, ScopeNote, Section, Stat, Table, WindowTabs } from "@/components/ui";
import { VolumeChart } from "@/components/volume-chart";
import { api, getNetwork, parseWindow } from "@/lib/api";
import { categoryLabel, count, ms, pct, rateLabel, usd, windowLabel } from "@/lib/format";

export const dynamic = "force-dynamic";

export default async function OverviewPage({
  searchParams,
}: {
  searchParams: Promise<{ window?: string }>;
}) {
  const hours = parseWindow((await searchParams).window);
  const [overview, recent, large, network] = await Promise.all([
    api.overview(hours),
    api.flows({ limit: 14 }),
    api.flows({ since_hours: hours, min_usd: 10_000, limit: 8 }),
    getNetwork(),
  ]);
  const { coverage, ingest, reference_rates: rates } = overview;
  const { scope } = coverage;
  const label = windowLabel(hours);

  return (
    <>
      <PageHeader
        title="Stablecoin flow on Arc, attributed"
        lede="Every token movement on Arc, decoded into the economic flow it represents and matched to the entity behind it. Listed assets are valued in USD; every other token is tracked by amount."
      >
        <WindowTabs basePath="/" current={hours} />
      </PageHeader>

      <ScopeNote scope={scope} hours={hours} />

      <section className="grid gap-6 md:grid-cols-[1.4fr_1fr]">
        <div className="rounded-sm border border-line-strong bg-surface p-6">
          <p className="text-xs text-ink-2">Headline claim, measured over the last {label}</p>
          {coverage.claim_volume_pct === null ? (
            <p className="mt-3 max-w-[52ch] text-lg leading-snug">
              No live flows measured yet. The claim is computed only from blocks produced while
              the ingestor is following the chain head.
            </p>
          ) : (
            <p className="mt-3 max-w-[52ch] text-lg leading-snug">
              <span className="font-mono text-4xl font-semibold tracking-tight">
                {pct(coverage.claim_volume_pct)}
              </span>{" "}
              of live volume is attributed to a known entity within{" "}
              {coverage.claim_within_seconds} seconds of settlement.
            </p>
          )}
          <Link
            href={`/coverage?window=${hours}`}
            className="mt-4 inline-block text-sm text-accent-ink underline underline-offset-2"
          >
            See how this is measured
          </Link>
        </div>
        <dl className="grid grid-cols-2 gap-x-6 gap-y-5">
          <Stat label="Median latency" value={ms(coverage.latency_p50_ms)} sub="settlement to queryable" />
          <Stat label="p95 latency" value={ms(coverage.latency_p95_ms)} sub={`${count(coverage.live_flows)} live flows`} />
          <Stat
            label="Ingest lag"
            value={ingest.lag_blocks === null ? "n/a" : `${count(ingest.lag_blocks)} blocks`}
            sub={ingest.running ? `head ${ingest.chain_head ?? "unknown"}` : "ingestor not running"}
          />
          <Stat
            label="Proprietary coverage"
            value={pct(coverage.institutional_volume_coverage_pct)}
            sub="volume, excluding protocol labels"
          />
        </dl>
      </section>

      {ingest.last_error ? (
        <p role="alert" className="rounded-sm border border-line-strong px-4 py-3 text-sm text-ink-2">
          The ingestor reported an error and is retrying: <span className="font-mono text-xs">{ingest.last_error}</span>
        </p>
      ) : null}

      <dl className="grid grid-cols-2 gap-6 md:grid-cols-4">
        <Stat label={`Volume, ${label}`} value={usd(overview.volume_usd)} sub="notional USD" />
        <Stat label="Flows" value={count(overview.flows)} />
        <Stat label="Active addresses" value={count(overview.active_addresses)} />
        <Stat
          label="Attributed volume"
          value={pct(coverage.volume_coverage_pct)}
          sub={`${pct(coverage.flow_coverage_pct)} of flows`}
        />
      </dl>

      <div className="grid gap-10 lg:grid-cols-[1.6fr_1fr]">
        <Section
          title="Volume by asset"
          note={`${overview.series_bucket_minutes} minute buckets, listed assets only. EURC at ${rateLabel(rates.eurc_usd, rates.eurc_source, rates.eurc_as_of)}, USYC at ${rateLabel(rates.usyc_usd, rates.usyc_source, rates.usyc_as_of)}. ${
            rates.btc_usd !== null && rates.eth_usd !== null
              ? `cirBTC and WETH at ${rates.spot_source} (${rates.spot_as_of}).`
              : "cirBTC and WETH are unpriced until a spot price arrives."
          }`}
        >
          <VolumeChart series={overview.series} bucketMinutes={overview.series_bucket_minutes} />
        </Section>
        <Section
          title="Assets"
          note={
            overview.asset_count > overview.assets.length ? (
              <Link href={`/assets?window=${hours}`} className="text-accent-ink underline underline-offset-2">
                All {count(overview.asset_count)} assets
              </Link>
            ) : undefined
          }
        >
          {overview.assets.length === 0 ? (
            <Empty title="No flows in this window" />
          ) : (
            <Table
              head={
                <tr>
                  <th>Asset</th>
                  <th className="text-right">Flows</th>
                  <th className="text-right">Volume</th>
                  {scope.shielding_live ? <th className="text-right">Shielded</th> : null}
                </tr>
              }
            >
              {overview.assets.map((asset) => (
                <tr key={asset.address}>
                  <td className="font-medium">
                    <AssetName symbol={asset.asset} address={asset.address} listed={asset.listed} />
                  </td>
                  <td className="text-right font-mono">{count(asset.flows)}</td>
                  <td className="text-right font-mono">
                    {asset.priced ? usd(asset.volume_usd) : <span className="text-ink-3">Unpriced</span>}
                  </td>
                  {scope.shielding_live ? (
                    <td className="text-right font-mono">{count(asset.shielded_flows)}</td>
                  ) : null}
                </tr>
              ))}
            </Table>
          )}
        </Section>
      </div>

      <div className="grid gap-10 lg:grid-cols-[1.6fr_1fr]">
        <Section title="Live flows" note="Updates every few seconds">
          <LiveFeed initial={recent.flows} />
        </Section>
        <div className="flex flex-col gap-10">
          <Section title="Most active entities">
            {overview.top_entities.length === 0 ? (
              <Empty title="No attributed activity in this window" />
            ) : (
              <Table
                head={
                  <tr>
                    <th>Entity</th>
                    <th className="text-right">In</th>
                    <th className="text-right">Out</th>
                  </tr>
                }
              >
                {overview.top_entities.map((row) => (
                  <tr key={row.entity.slug}>
                    <td>
                      <Link href={`/entities/${row.entity.slug}`} className="font-medium hover:text-accent-ink hover:underline">
                        {row.entity.name}
                      </Link>
                      <p className="text-xs text-ink-3">{categoryLabel(row.entity.category)}</p>
                    </td>
                    <td className="text-right font-mono">{usd(row.inflow_usd)}</td>
                    <td className="text-right font-mono">{usd(row.outflow_usd)}</td>
                  </tr>
                ))}
              </Table>
            )}
          </Section>
          <Section title="Cross-chain corridors">
            {overview.corridors.length === 0 ? (
              <Empty title="No CCTP or Gateway flows in this window" />
            ) : (
              <Table
                head={
                  <tr>
                    <th>Corridor</th>
                    <th>Rail</th>
                    <th className="text-right">Flows</th>
                    <th className="text-right">Volume</th>
                  </tr>
                }
              >
                {overview.corridors.map((c) => (
                  <tr key={`${c.protocol}-${c.direction}-${c.domain}`}>
                    <td className="font-medium">
                      {c.direction === "out" ? `Arc to ${c.chain}` : `${c.chain} to Arc`}
                    </td>
                    <td className="text-ink-2">{c.protocol === "cctp" ? "CCTP" : "Gateway"}</td>
                    <td className="text-right font-mono">{count(c.flows)}</td>
                    <td className="text-right font-mono">{usd(c.volume_usd)}</td>
                  </tr>
                ))}
              </Table>
            )}
          </Section>
        </div>
      </div>

      <Section title="Largest recent flows" note="Commercial size and above ($10K+)">
        {large.flows.length === 0 ? (
          <Empty title="No flows above $10K in this window">
            {network.key === "testnet" ? "Arc Testnet traffic is mostly small test transfers. " : ""}
            Lower the threshold on the{" "}
            <Link href="/flows" className="text-accent-ink underline underline-offset-2">
              flows page
            </Link>
            .
          </Empty>
        ) : (
          <FlowTable flows={large.flows} />
        )}
      </Section>
    </>
  );
}
