import Link from "next/link";
import type { AddressLabel } from "@tideline/sdk";
import { methodLabel, utc } from "@/lib/format";
import { Table, Tag } from "./ui";

/** Attribution claims with the proof behind each one. */
export function LabelTable({ labels, showEntity = false }: { labels: AddressLabel[]; showEntity?: boolean }) {
  return (
    <Table
      wide
      head={
        <tr>
          <th>{showEntity ? "Entity" : "Address"}</th>
          <th>Role</th>
          <th>Method</th>
          <th className="text-right">Confidence</th>
          <th>Evidence</th>
        </tr>
      }
    >
      {labels.map((label) => (
        <tr key={`${label.address}-${label.entity.slug}-${label.method}`}>
          <td>
            {showEntity ? (
              <Link prefetch={false} href={`/entities/${label.entity.slug}`} className="font-medium hover:text-accent-ink hover:underline">
                {label.entity.name}
              </Link>
            ) : (
              <Link prefetch={false} href={`/address/${label.address}`} className="font-mono text-xs hover:text-accent-ink hover:underline">
                {label.address}
              </Link>
            )}
          </td>
          <td>{label.label || <span className="text-ink-3">Unspecified</span>}</td>
          <td>
            <Tag tone={label.method === "heuristic" ? "muted" : "neutral"}>{methodLabel(label.method)}</Tag>
          </td>
          <td className="text-right font-mono">{label.confidence.toFixed(2)}</td>
          <td className="max-w-[40ch]">
            {label.evidence ? (
              <details>
                <summary className="cursor-pointer text-xs text-accent-ink">
                  {label.source || "View evidence"}
                </summary>
                <pre className="mt-2 overflow-x-auto whitespace-pre-wrap break-all rounded-sm border border-line bg-surface p-2 font-mono text-[11px] leading-relaxed text-ink-2">
                  {JSON.stringify(label.evidence, null, 2)}
                </pre>
                <p className="mt-1 text-[11px] text-ink-3">Recorded {utc(label.created_at)}</p>
              </details>
            ) : (
              <span className="text-xs text-ink-3">
                {label.source ? `${label.source}. ` : ""}Full evidence on the licensed tier.
              </span>
            )}
          </td>
        </tr>
      ))}
    </Table>
  );
}
