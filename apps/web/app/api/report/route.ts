import { NextResponse, type NextRequest } from "next/server";
import { TidelineError } from "@tideline/sdk";
import { api, parseWindow } from "@/lib/api";

/** CSV export of an entity's compliance report. */
export async function GET(request: NextRequest) {
  const entity = request.nextUrl.searchParams.get("entity");
  if (!entity || !/^[a-z0-9-]+$/.test(entity)) {
    return NextResponse.json({ error: "bad_request" }, { status: 400 });
  }
  const hours = parseWindow(request.nextUrl.searchParams.get("window") ?? undefined);
  try {
    const csv = await api.complianceReportCsv(entity, hours);
    return new NextResponse(csv, {
      headers: {
        "content-type": "text/csv; charset=utf-8",
        "content-disposition": `attachment; filename="tideline-${entity}-${hours}h.csv"`,
      },
    });
  } catch (error) {
    const status = error instanceof TidelineError ? error.status : 502;
    const message =
      status === 401 || status === 403
        ? "The compliance report needs a licensed API key in TIDELINE_API_KEY."
        : "The Tideline API did not return a report.";
    return NextResponse.json({ error: message }, { status });
  }
}
