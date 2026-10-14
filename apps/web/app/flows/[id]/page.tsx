import Link from "next/link";
import { notFound } from "next/navigation";
import type { Party } from "@tideline/sdk";
import { AssetName, PageHeader, Section, Tag, AttributionTag } from "@/components/ui";
import { api, getNetwork } from "@/lib/api";
import { categoryLabel, explorerTx, kindLabel, methodLabel, ms, usd, utc } from "@/lib/format";
import { orNotFound } from "@/lib/guard";

export const dynamic = "force-dynamic";
export const metadata = { title: "Flow" };

function PartyPanel({ side, party }: { side: string; party: Party }) {
  const isAddress = /^0x[0-9a-f]{40}$/.test(party.address);
  return (
    <div className="flex flex-col gap-2 rounded-sm border border-line-strong bg-surface p-5">
      <p className="text-xs text-ink-2">{side}</p>
      {party.entity ? (
        <>
          <Link href={`/entities/${party.entity.slug}`} className="text-lg font-semibold hover:text-accent-ink hover:underline">
            {party.entity.name}
          </Link>
          <p className="text-sm text-ink-2">
            {categoryLabel(party.entity.category)}
            {party.label ? `, ${party.label}` : ""}
          </p>
          <p className="flex flex-wrap items-center gap-2 text-xs text-ink-2">
            {party.method ? <Tag>{methodLabel(party.method)}</Tag> : null}
            {party.confidence !== null ? (
              <span className="font-mono">confidence {party.confidence.toFixed(2)}</span>
            ) : null}
          </p>
        </>
      ) : (
        <p className="text-lg font-semibold text-ink-2">Unattributed</p>
      )}
      {isAddress ? (
        <Link href={`/address/${party.address}`} className="break-all font-mono text-xs text-accent-ink underline underline-offset-2">
          {party.address}
        </Link>
      ) : (
        <p className="break-all font-mono text-xs text-ink-2">{party.address}</p>
      )}
    </div>
  );
}

export default async function FlowPage({ params }: { params: Promise<{ id: string }> }) {
  const id = Number((await params).id);
  if (!Number.isSafeInteger(id)) notFound();
  const [flow, network] = await Promise.all([orNotFound(api.flow(id)), getNetwork()]);

  const facts: [string, React.ReactNode][] = [
    ["Type", kindLabel(flow.kind)],
    ["Asset", <AssetName key="asset" symbol={flow.asset} address={flow.asset_address} listed={flow.asset_listed} />],
    ["Token contract", <span key="contract" className="break-all font-mono text-xs">{flow.asset_address}</span>],
    [
      "Amount",
      flow.token_id !== null
        ? `NFT #${flow.token_id}`
        : flow.amount
          ? `${flow.amount} ${flow.asset}`
          : "Shielded",
    ],
    [
      "Notional USD",
      flow.visibility === "shielded"
        ? "Shielded"
        : flow.usd_value === null
          ? flow.asset_listed
            ? "Not yet priced"
            : "Unpriced: unlisted token"
          : usd(flow.usd_value),
    ],
    [
      "Size band",
      flow.size_tier ?? (flow.visibility === "shielded" ? "Unknown while shielded" : "None without a USD value"),
    ],
    ["Visibility", flow.visibility],
    ["Settled", utc(flow.block_time)],
    ["Block", flow.block_number.toLocaleString("en-US")],
    ["Settlement to attributed", flow.latency_ms === null ? "Backfilled, not measured" : ms(flow.latency_ms)],
  ];
  if (flow.src_chain) facts.push(["Source chain", flow.src_chain]);
  if (flow.dst_chain) facts.push(["Destination chain", flow.dst_chain]);
  const details = Object.entries(flow.details ?? {});

  return (
    <>
      <PageHeader title={flow.narrative}>
        <AttributionTag attribution={flow.attribution} />
      </PageHeader>

      <div className="grid gap-4 md:grid-cols-2">
        <PartyPanel side="From" party={flow.from} />
        <PartyPanel side="To" party={flow.to} />
      </div>

      <div className="grid gap-10 lg:grid-cols-2">
        <Section title="Flow">
          <dl className="grid grid-cols-[12rem_1fr] gap-y-2 text-sm">
            {facts.map(([label, value]) => (
              <div key={label} className="contents">
                <dt className="text-ink-2">{label}</dt>
                <dd className="font-mono text-[13px]">{value}</dd>
              </div>
            ))}
            <dt className="text-ink-2">Transaction</dt>
            <dd>
              <a
                href={explorerTx(network.explorer_url, flow.tx_hash)}
                target="_blank"
                rel="noreferrer"
                className="break-all font-mono text-xs text-accent-ink underline underline-offset-2"
              >
                {flow.tx_hash}
              </a>
              <span className="ml-2 font-mono text-xs text-ink-3">log {flow.log_index}</span>
            </dd>
          </dl>
        </Section>
        <Section title="Decoded protocol context">
          {flow.details === null ? (
            <p className="text-sm text-ink-2">Decoded context is part of the licensed tier.</p>
          ) : details.length === 0 ? (
            <p className="text-sm text-ink-2">A plain token transfer. No protocol context to decode.</p>
          ) : (
            <dl className="grid grid-cols-[14rem_1fr] gap-y-2 text-sm">
              {details.map(([key, value]) => (
                <div key={key} className="contents">
                  <dt className="text-ink-2">{key.replaceAll("_", " ")}</dt>
                  <dd className="break-all font-mono text-[13px]">{String(value)}</dd>
                </div>
              ))}
            </dl>
          )}
        </Section>
      </div>
    </>
  );
}
