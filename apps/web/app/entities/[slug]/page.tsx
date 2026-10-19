import Link from "next/link";
import { LabelTable } from "@/components/evidence";
import { FlowTable } from "@/components/flow-table";
import { Code, Empty, PageHeader, PartyCell, ScopeNote, Section, Stat, Table, WindowTabs } from "@/components/ui";
import { api, parseWindow } from "@/lib/api";
import { categoryLabel, count, usd, windowLabel } from "@/lib/format";
import { orNotFound } from "@/lib/guard";

export const dynamic = "force-dynamic";

export async function generateMetadata({ params }: { params: Promise<{ slug: string }> }) {
  return { title: (await params).slug };
}

export default async function EntityPage({
  params,
  searchParams,
}: {
  params: Promise<{ slug: string }>;
  searchParams: Promise<{ window?: string }>;
}) {
  const { slug } = await params;
  const hours = parseWindow((await searchParams).window);
  const profile = await orNotFound(api.entity(slug, hours));
  const { entity } = profile;
  const label = windowLabel(hours);

  return (
    <>
      <PageHeader
        eyebrow="Entity"
        title={entity.name}
        lede={
          <>
            {categoryLabel(entity.category)}
            {entity.description ? `. ${entity.description}` : ""}
            {entity.source ? ` Registry source: ${entity.source}.` : ""}
          </>
        }
      >
        <WindowTabs basePath={`/entities/${slug}`} current={hours} />
      </PageHeader>

      {entity.status === "watchlist" ? (
        <Empty title={`No address is attributed to ${entity.name} yet`}>
          <p>
            Tideline does not guess. Attribute an address with evidence and every historical
            flow touching it is labelled at once.
          </p>
          <div className="mt-4">
            <Code>{`curl -X POST $TIDELINE_API_URL/v1/attributions \\
  -H "Authorization: Bearer $TIDELINE_ADMIN_KEY" -H "content-type: application/json" \\
  -d '{"entity_slug":"${slug}","address":"0x...","label":"Treasury",
       "confidence":0.9,"source":"Public disclosure",
       "evidence":{"url":"https://...","note":"why this address is theirs"}}'`}</Code>
          </div>
        </Empty>
      ) : (
        <>
          <ScopeNote scope={profile.scope} hours={hours} />

          <dl className="grid grid-cols-2 gap-6 md:grid-cols-4">
            <Stat label={`Inflow, ${label}`} value={usd(profile.inflow_usd)} />
            <Stat label={`Outflow, ${label}`} value={usd(profile.outflow_usd)} />
            <Stat label="Flows" value={count(profile.flows)} />
            {profile.scope.shielding_live ? (
              <Stat label="Amount-shielded flows" value={count(profile.shielded_flows)} sub="counted, excluded from USD totals" />
            ) : (
              <Stat label="Attributed addresses" value={count(entity.address_count)} />
            )}
          </dl>

          <Section title="Attributed addresses" note="Each claim carries its method, confidence and evidence">
            <LabelTable labels={profile.addresses} />
          </Section>

          <Section
            title={`Every flow touching ${entity.name}, last ${label}`}
            note={
              <a href={`/api/report?entity=${slug}&window=${hours}`} className="text-accent-ink underline underline-offset-2">
                Export compliance report (CSV)
              </a>
            }
          >
            {profile.recent_flows.length === 0 ? (
              <Empty title="No flows in this window" />
            ) : (
              <>
                <FlowTable flows={profile.recent_flows} />
                {profile.flows > profile.recent_flows.length ? (
                  <Link
                    prefetch={false}
                    href={`/flows?entity=${slug}`}
                    className="self-start text-sm text-accent-ink underline underline-offset-2"
                  >
                    View all {count(profile.flows)} flows
                  </Link>
                ) : null}
              </>
            )}
          </Section>

          <Section title="Top counterparties" note={`By volume, last ${label}`}>
            {profile.counterparties.length === 0 ? (
              <Empty title="No counterparties in this window" />
            ) : (
              <Table
                head={
                  <tr>
                    <th>Counterparty</th>
                    <th className="text-right">Flows</th>
                    <th className="text-right">Volume</th>
                  </tr>
                }
              >
                {profile.counterparties.map((c) => (
                  <tr key={c.address}>
                    <td>
                      <PartyCell
                        party={{ address: c.address, entity: c.entity, label: null, method: null, confidence: null }}
                      />
                    </td>
                    <td className="text-right font-mono">{count(c.flows)}</td>
                    <td className="text-right font-mono">{usd(c.volume_usd)}</td>
                  </tr>
                ))}
              </Table>
            )}
          </Section>
        </>
      )}
    </>
  );
}
