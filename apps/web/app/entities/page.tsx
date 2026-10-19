import Link from "next/link";
import { Empty, PageHeader, Section, Table } from "@/components/ui";
import { api } from "@/lib/api";
import { categoryLabel, count, usd } from "@/lib/format";

export const dynamic = "force-dynamic";
export const metadata = { title: "Entities" };

export default async function EntitiesPage({
  searchParams,
}: {
  searchParams: Promise<{ q?: string }>;
}) {
  const q = ((await searchParams).q ?? "").trim().toLowerCase();
  const all = await api.entities();
  const entities = q
    ? all.filter((e) => e.name.toLowerCase().includes(q) || e.slug.includes(q))
    : all;
  const attributed = entities.filter((e) => e.status === "attributed");
  const watchlist = entities.filter((e) => e.status === "watchlist");

  return (
    <>
      <PageHeader
        eyebrow="Attribution registry"
        title="Entities"
        lede="The attribution registry: who is behind the addresses moving value on Arc, and the evidence for each claim."
      />

      {entities.length === 0 ? (
        <Empty title={`No entity matches "${q}"`}>
          <Link prefetch={false} href="/entities" className="text-accent-ink underline underline-offset-2">
            Show all entities
          </Link>
        </Empty>
      ) : null}

      {attributed.length > 0 ? (
        <Section title="Attributed" note="At least one address on file">
          <Table
            head={
              <tr>
                <th>Entity</th>
                <th>Category</th>
                <th className="text-right">Addresses</th>
                <th className="text-right">Flows, 24h</th>
                <th className="text-right">Volume, 24h</th>
              </tr>
            }
          >
            {attributed.map((entity) => (
              <tr key={entity.slug}>
                <td>
                  <Link prefetch={false} href={`/entities/${entity.slug}`} className="font-medium hover:text-accent-ink hover:underline">
                    {entity.name}
                  </Link>
                </td>
                <td className="text-ink-2">{categoryLabel(entity.category)}</td>
                <td className="text-right font-mono">{count(entity.address_count)}</td>
                <td className="text-right font-mono">{count(entity.flows_24h)}</td>
                <td className="text-right font-mono">{usd(entity.volume_usd_24h)}</td>
              </tr>
            ))}
          </Table>
        </Section>
      ) : null}

      {watchlist.length > 0 ? (
        <Section title="Watchlist" note="Named Arc participants with no address attributed yet">
          <p className="max-w-[65ch] text-sm leading-relaxed text-ink-2">
            These institutions are publicly associated with Arc. Tideline shows nothing for them
            until an address is attributed with evidence, by signed self-attestation or by
            curation. Their flows are labelled retroactively the moment that happens.
          </p>
          <ul className="grid grid-cols-2 gap-x-6 gap-y-3 md:grid-cols-3 lg:grid-cols-5">
            {watchlist.map((entity) => (
              <li key={entity.slug} className="border-t border-line pt-2">
                <Link prefetch={false} href={`/entities/${entity.slug}`} className="text-sm font-medium hover:text-accent-ink hover:underline">
                  {entity.name}
                </Link>
                <p className="text-xs text-ink-3">{categoryLabel(entity.category)}</p>
              </li>
            ))}
          </ul>
        </Section>
      ) : null}
    </>
  );
}
