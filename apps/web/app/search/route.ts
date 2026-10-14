import { redirect } from "next/navigation";
import type { NextRequest } from "next/server";
import { api } from "@/lib/api";
import { isEvmAddress } from "@/lib/format";

/**
 * Nav search. An address goes to its profile; a name filters the entity list, or the
 * asset list when it names a token and no entity.
 */
export async function GET(request: NextRequest) {
  const q = (request.nextUrl.searchParams.get("q") ?? "").trim();
  if (isEvmAddress(q)) redirect(`/address/${q.toLowerCase()}`);
  if (!q) redirect("/entities");
  const needle = q.toLowerCase();
  const [entities, assets] = await Promise.all([
    api.entities().catch(() => []),
    api.assets(24).catch(() => null),
  ]);
  const entityHit = entities.some((e) => e.name.toLowerCase().includes(needle) || e.slug.includes(needle));
  const assetHit = assets?.assets.some(
    (a) => a.symbol?.toLowerCase().includes(needle) || a.name?.toLowerCase().includes(needle),
  );
  redirect(!entityHit && assetHit ? `/assets?q=${encodeURIComponent(q)}` : `/entities?q=${encodeURIComponent(q)}`);
}
