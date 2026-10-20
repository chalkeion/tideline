import Link from "next/link";
import { LabelTable } from "@/components/evidence";
import { FlowTable } from "@/components/flow-table";
import { Empty, PageHeader, Section, Stat, Table } from "@/components/ui";
import { api, getNetwork } from "@/lib/api";
import { count, explorerTx, usd } from "@/lib/format";
import { orNotFound } from "@/lib/guard";

export const revalidate = 300;

// No pages at build time (the API may be asleep); each address renders on first request, then caches.
export function generateStaticParams() {
  return [];
}
export const metadata = { title: "Address" };

const LINK_TYPES: Record<string, string> = {
  gateway_delegate: "Signed a Gateway burn spending this balance, or had one signed for it",
};

export default async function AddressPage({ params }: { params: Promise<{ address: string }> }) {
  const { address } = await params;
  const [profile, network, token] = await Promise.all([
    orNotFound(api.address(address)),
    getNetwork(),
    // Most addresses are not token contracts.
    api.asset(address).catch(() => null),
  ]);
  const best = profile.labels[0];

  return (
    <>
      <PageHeader
        eyebrow="Address"
        title={best ? best.entity.name : "Unattributed address"}
        lede={<span className="break-all font-mono text-xs">{profile.address}</span>}
      />

      {token ? (
        <p role="note" className="rounded-sm border border-line-strong px-4 py-3 text-sm leading-relaxed text-ink-2">
          This address is the contract of {token.asset.listed ? "the listed token" : "an unlisted token"}{" "}
          <Link prefetch={false} href={`/assets/${token.asset.address}`} className="font-medium text-accent-ink underline underline-offset-2">
            {token.asset.symbol ?? "without a symbol"}
          </Link>
          . Its transfers are on the asset page.
        </p>
      ) : null}

      <dl className="grid grid-cols-2 gap-6 md:grid-cols-3">
        <Stat label="Flows on file" value={count(profile.flows)} />
        <Stat label="Inflow" value={usd(profile.inflow_usd)} />
        <Stat label="Outflow" value={usd(profile.outflow_usd)} />
      </dl>

      <Section title="Attribution">
        {profile.labels.length === 0 ? (
          <Empty title="No attribution on file">
            Tideline has no evidence tying this address to an entity. It appears on the{" "}
            <Link prefetch={false} href="/coverage" className="text-accent-ink underline underline-offset-2">
              coverage worklist
            </Link>{" "}
            if it moves meaningful volume.
          </Empty>
        ) : (
          <LabelTable labels={profile.labels} showEntity />
        )}
      </Section>

      {profile.links.length > 0 ? (
        <Section title="Linked addresses" note="Control relationships observed on-chain">
          <Table
            head={
              <tr>
                <th>Address</th>
                <th>Relationship</th>
                <th className="text-right">Seen</th>
                <th>Evidence</th>
              </tr>
            }
          >
            {profile.links.map((link) => (
              <tr key={`${link.address}-${link.link_type}`}>
                <td>
                  <Link prefetch={false} href={`/address/${link.address}`} className="font-mono text-xs hover:text-accent-ink hover:underline">
                    {link.address}
                  </Link>
                </td>
                <td className="text-ink-2">{LINK_TYPES[link.link_type] ?? link.link_type}</td>
                <td className="text-right font-mono">{count(link.seen_count)}</td>
                <td>
                  <a
                    href={explorerTx(network.explorer_url, link.evidence_tx)}
                    target="_blank"
                    rel="noreferrer"
                    className="text-xs text-accent-ink underline underline-offset-2"
                  >
                    Linking transaction
                  </a>
                </td>
              </tr>
            ))}
          </Table>
        </Section>
      ) : null}

      <Section title="Recent flows">
        {profile.recent_flows.length === 0 ? (
          <Empty title="No flows on file for this address" />
        ) : (
          <>
            <FlowTable flows={profile.recent_flows} />
            {profile.flows > profile.recent_flows.length ? (
              <Link
                prefetch={false}
                href={`/flows?address=${profile.address}`}
                className="self-start text-sm text-accent-ink underline underline-offset-2"
              >
                View all {count(profile.flows)} flows
              </Link>
            ) : null}
          </>
        )}
      </Section>
    </>
  );
}
