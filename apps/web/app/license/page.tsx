import { Code, PageHeader, Section, Table } from "@/components/ui";

export const metadata = { title: "Data license" };

const TIERS = [
  {
    name: "Public",
    auth: "No key",
    limit: "120 requests / minute",
    includes: "Overview, coverage, entity registry, labelled flows from the last 24 hours (50 per page).",
  },
  {
    name: "Licensed",
    auth: "Bearer key",
    limit: "1,200 requests / minute, metered per key per day",
    includes:
      "Attribution evidence per address, decoded protocol context per flow, 30 days of history (1,000 per page), the compliance report in JSON and CSV, and the live flow stream.",
  },
  {
    name: "Admin",
    auth: "Bearer key",
    limit: "Unmetered",
    includes: "Everything licensed, plus curating the registry: attributions, signed attestations, disclosures and key issuance.",
  },
];

export default function LicensePage() {
  return (
    <>
      <PageHeader
        title="Data license"
        lede="Tideline is sold as data, not as a token. Compliance and risk teams license the attribution and enrichment layer through this API."
      />

      <Section title="Access tiers">
        <Table
          head={
            <tr>
              <th>Tier</th>
              <th>Auth</th>
              <th>Limits</th>
              <th>Includes</th>
            </tr>
          }
        >
          {TIERS.map((tier) => (
            <tr key={tier.name}>
              <td className="font-medium">{tier.name}</td>
              <td className="whitespace-nowrap text-ink-2">{tier.auth}</td>
              <td className="text-ink-2">{tier.limit}</td>
              <td className="max-w-[60ch] text-ink-2">{tier.includes}</td>
            </tr>
          ))}
        </Table>
      </Section>

      <div className="grid gap-10 lg:grid-cols-2">
        <Section title="The compliance question, as one request">
          <p className="max-w-[65ch] text-sm leading-relaxed text-ink-2">
            Every flow touching an entity in the last 24 hours, with the method, confidence and
            evidence behind each attributed address.
          </p>
          <Code>{`curl "$TIDELINE_API_URL/v1/compliance/report?entity=circle&window_hours=24" \\
  -H "Authorization: Bearer $TIDELINE_API_KEY"

# The same report as CSV
curl "$TIDELINE_API_URL/v1/compliance/report?entity=circle&format=csv" \\
  -H "Authorization: Bearer $TIDELINE_API_KEY" -o circle-24h.csv`}</Code>
        </Section>
        <Section title="Live stream">
          <p className="max-w-[65ch] text-sm leading-relaxed text-ink-2">
            Server-sent events, one per flow, as soon as it is attributed.
          </p>
          <Code>{`curl -N "$TIDELINE_API_URL/v1/flows/stream" \\
  -H "Authorization: Bearer $TIDELINE_API_KEY"

event: flow
id: 48211
data: {"id":48211,"kind":"cctp_outbound","asset":"USDC", ...}`}</Code>
        </Section>
        <Section title="TypeScript SDK">
          <Code>{`import { TidelineClient } from "@tideline/sdk";

const tideline = new TidelineClient({
  baseUrl: process.env.TIDELINE_API_URL!,
  apiKey: process.env.TIDELINE_API_KEY,
});

const { flows } = await tideline.flows({ entity: "circle", min_usd: 1_000_000 });
const report = await tideline.complianceReport("circle", 24);`}</Code>
        </Section>
        <Section title="Issue a key">
          <p className="max-w-[65ch] text-sm leading-relaxed text-ink-2">
            Keys are shown once. Tideline stores only a hash.
          </p>
          <Code>{`# From the service host
cargo run -- keys create "Risk desk" licensed

# Or over the API, as an admin
curl -X POST "$TIDELINE_API_URL/v1/keys" \\
  -H "Authorization: Bearer $TIDELINE_ADMIN_KEY" -H "content-type: application/json" \\
  -d '{"name":"Risk desk","tier":"licensed"}'`}</Code>
        </Section>
      </div>
    </>
  );
}
