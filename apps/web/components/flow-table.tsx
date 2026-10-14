import Link from "next/link";
import type { Flow } from "@tideline/sdk";
import { kindLabel, usd, utc } from "@/lib/format";
import { AttributionTag, PartyCell, Table } from "./ui";

export function FlowAmount({ flow }: { flow: Flow }) {
  if (flow.visibility === "shielded") {
    return (
      <span className="shielded-hatch inline-block rounded-sm border border-line-strong px-1.5 py-0.5 text-[11px] leading-none text-ink-2">
        Amount shielded
      </span>
    );
  }
  return (
    <span className="flex flex-col items-end gap-0.5">
      <span className="font-mono">{usd(flow.usd_value)}</span>
      <span className="font-mono text-xs text-ink-3">
        {flow.amount} {flow.asset}
        {flow.visibility === "disclosed" ? " (disclosed)" : ""}
      </span>
    </span>
  );
}

export function FlowTable({ flows }: { flows: Flow[] }) {
  return (
    <Table
      wide
      head={
        <tr>
          <th>Time</th>
          <th>Flow</th>
          <th>From</th>
          <th>To</th>
          <th className="text-right">Value</th>
          <th>Attribution</th>
        </tr>
      }
    >
      {flows.map((flow) => (
        <tr key={flow.id}>
          <td className="whitespace-nowrap font-mono text-xs text-ink-2">{utc(flow.block_time)}</td>
          <td>
            <Link
              href={`/flows/${flow.id}`}
              className="font-medium underline-offset-2 hover:text-accent-ink hover:underline"
            >
              {kindLabel(flow.kind)}
            </Link>
            <p className="mt-0.5 max-w-[44ch] text-xs leading-snug text-ink-3">{flow.narrative}</p>
          </td>
          <td>
            <PartyCell party={flow.from} />
          </td>
          <td>
            <PartyCell party={flow.to} />
          </td>
          <td className="text-right">
            <FlowAmount flow={flow} />
          </td>
          <td>
            <AttributionTag attribution={flow.attribution} />
          </td>
        </tr>
      ))}
    </Table>
  );
}
