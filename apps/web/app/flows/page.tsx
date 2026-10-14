import Link from "next/link";
import type { FlowQuery } from "@tideline/sdk";
import { FlowTable } from "@/components/flow-table";
import { Empty, PageHeader } from "@/components/ui";
import { api } from "@/lib/api";
import { KINDS, kindLabel, shortAddress } from "@/lib/format";

export const dynamic = "force-dynamic";
export const metadata = { title: "Flows" };

type Search = Record<string, string | undefined>;

const SELECT =
  "h-9 rounded-sm border border-line-strong bg-surface px-2 text-sm text-ink";

function Field({ label, htmlFor, children }: { label: string; htmlFor: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-2">
      <label htmlFor={htmlFor} className="text-xs text-ink-2">
        {label}
      </label>
      {children}
    </div>
  );
}

export default async function FlowsPage({ searchParams }: { searchParams: Promise<Search> }) {
  const params = await searchParams;
  const minUsd = Number(params.min_usd);
  const before = Number(params.before);
  const query: FlowQuery = {
    asset: params.asset || undefined,
    kind: params.kind || undefined,
    entity: params.entity || undefined,
    address: params.address || undefined,
    attributed: params.attributed === "yes" ? true : params.attributed === "no" ? false : undefined,
    min_usd: Number.isFinite(minUsd) && minUsd > 0 ? minUsd : undefined,
    before: Number.isSafeInteger(before) && before > 0 ? before : undefined,
    limit: 50,
  };
  const [page, entities, assets] = await Promise.all([
    api.flows(query),
    api.entities(),
    api.assets(24),
  ]);
  // Listed tokens, then the busiest unlisted ones. A symbol in the URL names a listed token.
  const assetOptions = [
    ...assets.assets.filter((a) => a.listed),
    ...assets.assets.filter((a) => !a.listed && a.flows > 0).slice(0, 40),
  ];
  const selectedAsset =
    assetOptions.find((a) => a.address === params.asset?.toLowerCase())?.address ??
    assetOptions.find((a) => a.listed && a.symbol?.toLowerCase() === params.asset?.toLowerCase())?.address ??
    "";

  const older = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value && key !== "before") older.set(key, value);
  }
  if (page.next_cursor) older.set("before", String(page.next_cursor));
  const filtered = Object.entries(params).some(([key, value]) => value && key !== "before");

  return (
    <>
      <PageHeader
        title="Flows"
        lede="One row per economic flow. A bridge or a Gateway deposit is a single flow from its real originator, not the token transfers it was made of."
      />

      <form method="get" className="flex flex-wrap items-end gap-4">
        <Field label="Asset" htmlFor="asset">
          <select id="asset" name="asset" defaultValue={selectedAsset} className={`${SELECT} max-w-56`}>
            <option value="">All assets</option>
            {assetOptions.map((asset) => (
              <option key={asset.address} value={asset.address}>
                {asset.symbol ?? shortAddress(asset.address)}
                {asset.listed ? "" : " (unlisted)"}
              </option>
            ))}
          </select>
        </Field>
        <Field label="Flow type" htmlFor="kind">
          <select id="kind" name="kind" defaultValue={params.kind ?? ""} className={SELECT}>
            <option value="">All types</option>
            {KINDS.map((kind) => (
              <option key={kind} value={kind}>
                {kindLabel(kind)}
              </option>
            ))}
          </select>
        </Field>
        <Field label="Entity" htmlFor="entity">
          <select id="entity" name="entity" defaultValue={params.entity ?? ""} className={SELECT}>
            <option value="">Any entity</option>
            {entities
              .filter((entity) => entity.address_count > 0)
              .map((entity) => (
                <option key={entity.slug} value={entity.slug}>
                  {entity.name}
                </option>
              ))}
          </select>
        </Field>
        <Field label="Attribution" htmlFor="attributed">
          <select id="attributed" name="attributed" defaultValue={params.attributed ?? ""} className={SELECT}>
            <option value="">Any</option>
            <option value="yes">Attributed</option>
            <option value="no">Unattributed</option>
          </select>
        </Field>
        <Field label="Minimum USD" htmlFor="min_usd">
          <input
            id="min_usd"
            name="min_usd"
            type="number"
            min="0"
            step="any"
            inputMode="decimal"
            defaultValue={params.min_usd ?? ""}
            className={`${SELECT} w-32 font-mono`}
          />
        </Field>
        <Field label="Address" htmlFor="address">
          <input
            id="address"
            name="address"
            type="text"
            spellCheck={false}
            defaultValue={params.address ?? ""}
            className={`${SELECT} w-72 font-mono text-xs`}
          />
        </Field>
        <button
          type="submit"
          className="h-9 rounded-sm bg-ink px-4 text-sm font-medium text-page active:translate-y-px"
        >
          Apply filters
        </button>
        {filtered ? (
          <Link href="/flows" className="h-9 py-2 text-sm text-accent-ink underline underline-offset-2">
            Clear
          </Link>
        ) : null}
      </form>

      {page.flows.length === 0 ? (
        <Empty title={filtered ? "No flows match these filters" : "No flows ingested yet"}>
          {filtered
            ? "Widen the filters, or clear them to see everything."
            : "Start the stream service. Flows appear as soon as the ingestor reaches the chain head."}
        </Empty>
      ) : (
        <>
          <FlowTable flows={page.flows} />
          {page.next_cursor ? (
            <Link
              href={`/flows?${older}`}
              className="self-start rounded-sm border border-line-strong px-3 py-1.5 text-sm hover:bg-accent-wash hover:text-accent-ink active:translate-y-px"
            >
              Older flows
            </Link>
          ) : null}
        </>
      )}
    </>
  );
}
