import Link from "next/link";
import { notFound } from "next/navigation";
import { FlowTable } from "@/components/flow-table";
import { Empty, PageHeader, PartyCell, Section, Stat, Table, WindowTabs } from "@/components/ui";
import { api, getNetwork, parseWindow } from "@/lib/api";
import {
  count,
  isEvmAddress,
  priceLabel,
  shortAddress,
  standardLabel,
  tokenAmount,
  usd,
  windowLabel,
} from "@/lib/format";
import { orNotFound } from "@/lib/guard";

export const revalidate = 300;

export async function generateMetadata({ params }: { params: Promise<{ address: string }> }) {
  return { title: shortAddress((await params).address) };
}

export default async function AssetPage({
  params,
  searchParams,
}: {
  params: Promise<{ address: string }>;
  searchParams: Promise<{ window?: string }>;
}) {
  const address = (await params).address.toLowerCase();
  if (!isEvmAddress(address)) notFound();
  const hours = parseWindow((await searchParams).window);
  const [profile, flows, network] = await Promise.all([
    orNotFound(api.asset(address, hours)),
    api.flows({ asset: address, limit: 25 }),
    getNetwork(),
  ]);
  const { asset } = profile;
  const symbol = asset.symbol ?? shortAddress(asset.address);
  const label = windowLabel(hours);
  const nft = asset.standard === "erc721";
  // The listed token this one is named after, to point readers at the real one.
  const original = asset.symbol_conflict
    ? (await api.assets(hours)).assets.find(
        (a) => a.listed && a.symbol?.toLowerCase() === asset.symbol?.toLowerCase(),
      )
    : undefined;

  const facts: [string, React.ReactNode][] = [
    [
      "Contract",
      <a
        key="contract"
        href={`${network.explorer_url}/token/${asset.address}`}
        target="_blank"
        rel="noreferrer"
        className="break-all font-mono text-xs text-accent-ink underline underline-offset-2"
      >
        {asset.address}
      </a>,
    ],
    ["Standard", standardLabel(asset.standard)],
    ["Name", asset.name ?? "Not reported by the contract"],
    ["Decimals", nft ? "None (each token is one item)" : (asset.decimals?.toString() ?? "Not reported: amounts are raw units")],
    ["Status", asset.listed ? "Listed: address published by Arc or the issuer" : "Unlisted: discovered from its own transfers"],
    ["Valued at", priceLabel(asset.price_source)],
  ];
  if (asset.issuer) {
    facts.push([
      "Issuer",
      <Link prefetch={false} key="issuer" href={`/entities/${asset.issuer.slug}`} className="text-accent-ink underline underline-offset-2">
        {asset.issuer.name}
      </Link>,
    ]);
  }
  if (asset.first_seen_block !== null) {
    facts.push(["First seen", `Block ${asset.first_seen_block.toLocaleString("en-US")}`]);
  }

  return (
    <>
      <PageHeader
        eyebrow="Asset"
        title={symbol}
        lede={
          asset.listed
            ? `${asset.name ?? symbol}, valued at ${priceLabel(asset.price_source).toLowerCase()}.`
            : "An unlisted token. Tideline tracks every transfer of it by amount and does not value it in USD."
        }
      >
        <WindowTabs basePath={`/assets/${asset.address}`} current={hours} />
      </PageHeader>

      {asset.symbol_conflict ? (
        <p role="note" className="rounded-sm border border-line-strong px-4 py-3 text-sm leading-relaxed text-ink-2">
          This is not the listed {asset.symbol}. It is a different contract that uses the same symbol.
          {original ? (
            <>
              {" "}
              The listed {original.symbol} is{" "}
              <Link prefetch={false} href={`/assets/${original.address}`} className="font-mono text-xs text-accent-ink underline underline-offset-2">
                {original.address}
              </Link>
              .
            </>
          ) : null}
        </p>
      ) : null}

      <dl className="grid grid-cols-2 gap-6 md:grid-cols-4">
        <Stat label={`Flows, ${label}`} value={count(asset.flows)} />
        <Stat
          label={nft ? `Items moved, ${label}` : `Moved, ${label}`}
          value={tokenAmount(asset.amount)}
          sub={asset.decimals === null && !nft ? "raw units" : symbol}
        />
        <Stat
          label={`Volume, ${label}`}
          value={asset.volume_usd === null ? "Unpriced" : usd(asset.volume_usd)}
          sub={asset.price_source ? priceLabel(asset.price_source) : "Only listed tokens are valued"}
        />
        <Stat label="Active addresses" value={count(asset.active_addresses)} />
      </dl>

      <div className="grid gap-10 lg:grid-cols-2">
        <Section title="Token">
          <dl className="grid grid-cols-[9rem_1fr] gap-y-2 text-sm">
            {facts.map(([name, value]) => (
              <div key={name} className="contents">
                <dt className="text-ink-2">{name}</dt>
                <dd>{value}</dd>
              </div>
            ))}
          </dl>
        </Section>

        <Section title="Most active addresses" note={`By ${symbol} moved, last ${label}`}>
          {profile.top_addresses.length === 0 ? (
            <Empty title="No transfers in this window" />
          ) : (
            <Table
              head={
                <tr>
                  <th>Address</th>
                  <th className="text-right">Flows</th>
                  <th className="text-right">Sent</th>
                  <th className="text-right">Received</th>
                </tr>
              }
            >
              {profile.top_addresses.map((party) => (
                <tr key={party.address}>
                  <td>
                    <PartyCell
                      party={{ address: party.address, entity: party.entity, label: null, method: null, confidence: null }}
                    />
                  </td>
                  <td className="text-right font-mono">{count(party.flows)}</td>
                  <td className="text-right font-mono" title={party.sent}>
                    {tokenAmount(party.sent)}
                  </td>
                  <td className="text-right font-mono" title={party.received}>
                    {tokenAmount(party.received)}
                  </td>
                </tr>
              ))}
            </Table>
          )}
        </Section>
      </div>

      <Section
        title={`Recent ${symbol} flows`}
        note={
          flows.next_cursor ? (
            <Link prefetch={false} href={`/flows?asset=${asset.address}`} className="text-accent-ink underline underline-offset-2">
              All flows
            </Link>
          ) : undefined
        }
      >
        {flows.flows.length === 0 ? <Empty title="No flows on file" /> : <FlowTable flows={flows.flows} />}
      </Section>
    </>
  );
}
