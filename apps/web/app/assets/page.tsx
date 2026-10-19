import Link from "next/link";
import { AssetName, Empty, PageHeader, Section, Table, Tag, WindowTabs } from "@/components/ui";
import { api, parseWindow } from "@/lib/api";
import { count, priceLabel, shortAddress, standardLabel, tokenAmount, usd, windowLabel } from "@/lib/format";

export const dynamic = "force-dynamic";
export const metadata = { title: "Assets" };

export default async function AssetsPage({
  searchParams,
}: {
  searchParams: Promise<{ window?: string; q?: string }>;
}) {
  const search = await searchParams;
  const hours = parseWindow(search.window);
  const q = (search.q ?? "").trim().toLowerCase();
  const { assets: all } = await api.assets(hours);
  const assets = q
    ? all.filter(
        (a) =>
          a.symbol?.toLowerCase().includes(q) || a.name?.toLowerCase().includes(q) || a.address.includes(q),
      )
    : all;
  const listed = assets.filter((a) => a.listed);
  const discovered = assets.filter((a) => !a.listed);
  const label = windowLabel(hours);
  const impostors = discovered.filter((a) => a.symbol_conflict).length;

  return (
    <>
      <PageHeader
        eyebrow="Token registry"
        title="Assets"
        lede="Every token that moves on Arc, identified by its contract address. Listed tokens are the ones whose address Arc or the issuer publishes, and only they are valued in USD. Any other token can take any name, so one named like a listed token is flagged."
      >
        <WindowTabs basePath="/assets" current={hours} />
      </PageHeader>

      {q && assets.length === 0 ? (
        <Empty title={`No asset matches "${search.q}"`}>
          <Link prefetch={false} href="/assets" className="text-accent-ink underline underline-offset-2">
            Show all assets
          </Link>
        </Empty>
      ) : null}

      {listed.length > 0 ? (
        <Section title="Listed" note={`Published addresses, valued in USD. Last ${label}`}>
          <Table
            wide
            head={
              <tr>
                <th>Asset</th>
                <th>Issuer</th>
                <th>Valued at</th>
                <th className="text-right">Flows</th>
                <th className="text-right">Moved</th>
                <th className="text-right">Volume</th>
                <th className="text-right">Addresses</th>
              </tr>
            }
          >
            {listed.map((asset) => (
              <tr key={asset.address}>
                <td>
                  <span className="font-medium">
                    <AssetName symbol={asset.symbol ?? shortAddress(asset.address)} address={asset.address} listed />
                  </span>
                  <p className="text-xs text-ink-3">{asset.name}</p>
                </td>
                <td className="text-ink-2">
                  {asset.issuer ? (
                    <Link prefetch={false} href={`/entities/${asset.issuer.slug}`} className="hover:text-accent-ink hover:underline">
                      {asset.issuer.name}
                    </Link>
                  ) : (
                    "Bridged"
                  )}
                </td>
                <td className="text-ink-2">{priceLabel(asset.price_source)}</td>
                <td className="text-right font-mono">{count(asset.flows)}</td>
                <td className="text-right font-mono" title={asset.amount}>
                  {tokenAmount(asset.amount)}
                </td>
                <td className="text-right font-mono">
                  {asset.volume_usd === null ? <span className="text-ink-3">Not yet priced</span> : usd(asset.volume_usd)}
                </td>
                <td className="text-right font-mono">{count(asset.active_addresses)}</td>
              </tr>
            ))}
          </Table>
        </Section>
      ) : null}

      {discovered.length > 0 ? (
        <Section
          title="Discovered"
          note={`${count(discovered.length)} unlisted tokens moved in the last ${label}${
            impostors > 0 ? `, ${count(impostors)} named like a listed token` : ""
          }. Tracked by amount, not valued.`}
        >
          <Table
            wide
            head={
              <tr>
                <th>Token</th>
                <th>Standard</th>
                <th>Contract</th>
                <th className="text-right">Flows</th>
                <th className="text-right">Moved</th>
                <th className="text-right">Addresses</th>
              </tr>
            }
          >
            {discovered.map((asset) => (
              <tr key={asset.address}>
                <td>
                  <span className="flex flex-wrap items-center gap-2">
                    <Link
                      prefetch={false}
                      href={`/assets/${asset.address}`}
                      className="font-medium underline-offset-2 hover:text-accent-ink hover:underline"
                    >
                      {asset.symbol ?? <span className="text-ink-3">No symbol</span>}
                    </Link>
                    {asset.symbol_conflict ? <Tag>Not the listed {asset.symbol}</Tag> : null}
                  </span>
                  {asset.name ? (
                    <p className="max-w-[32ch] truncate text-xs text-ink-3" title={asset.name}>
                      {asset.name}
                    </p>
                  ) : null}
                </td>
                <td className="text-ink-2">{standardLabel(asset.standard)}</td>
                <td>
                  <Link
                    prefetch={false}
                    href={`/assets/${asset.address}`}
                    className="font-mono text-xs text-ink-2 hover:text-accent-ink hover:underline"
                    title={asset.address}
                  >
                    {shortAddress(asset.address)}
                  </Link>
                </td>
                <td className="text-right font-mono">{count(asset.flows)}</td>
                <td className="text-right font-mono" title={asset.amount}>
                  {asset.decimals === null ? <span className="text-ink-3">{tokenAmount(asset.amount)} raw</span> : tokenAmount(asset.amount)}
                </td>
                <td className="text-right font-mono">{count(asset.active_addresses)}</td>
              </tr>
            ))}
          </Table>
        </Section>
      ) : null}
    </>
  );
}
