import { redirect } from "next/navigation";
import type { NextRequest } from "next/server";
import { isEvmAddress } from "@/lib/format";

/** Nav search: an address goes to its profile, anything else filters the entity list. */
export function GET(request: NextRequest) {
  const q = (request.nextUrl.searchParams.get("q") ?? "").trim();
  if (isEvmAddress(q)) redirect(`/address/${q.toLowerCase()}`);
  redirect(q ? `/entities?q=${encodeURIComponent(q)}` : "/entities");
}
