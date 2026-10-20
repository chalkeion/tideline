import { NextResponse, type NextRequest } from "next/server";
import { api } from "@/lib/api";

/** Live-feed poll: flows newer than `after`, newest first. Keeps the license key server-side. */
export async function GET(request: NextRequest) {
  const after = Number(request.nextUrl.searchParams.get("after") ?? 0);
  if (!Number.isSafeInteger(after) || after < 0) {
    return NextResponse.json({ error: "bad_request" }, { status: 400 });
  }
  try {
    const page = await api.flows({ after, limit: 14 });
    // Viewers polling from the same cursor share one function call per window at the CDN.
    return NextResponse.json(page.flows, {
      headers: { "Cache-Control": "public, s-maxage=60, stale-while-revalidate=120" },
    });
  } catch {
    return NextResponse.json({ error: "upstream_unavailable" }, { status: 502 });
  }
}
