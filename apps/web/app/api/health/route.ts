import { NextResponse } from "next/server";

export function GET() {
  return NextResponse.json({ ok: true, service: "tideline-web" }, {
    headers: { "Cache-Control": "public, s-maxage=60, stale-while-revalidate=120" },
  });
}
