"use client";

import Link from "next/link";
import { useEffect, useRef, useState } from "react";
import type { Flow } from "@tideline/sdk";
import { kindLabel, usd, utcTime } from "@/lib/format";

const POLL_MS = 4000;
const MAX_ROWS = 14;

/** Tails newly attributed flows by polling for ids above the last one seen. */
export function LiveFeed({ initial }: { initial: Flow[] }) {
  const [flows, setFlows] = useState(initial);
  const [fresh, setFresh] = useState<Set<number>>(new Set());
  const [error, setError] = useState(false);
  const lastId = useRef(initial[0]?.id ?? 0);

  useEffect(() => {
    let cancelled = false;
    const tick = async () => {
      try {
        const res = await fetch(`/api/flows?after=${lastId.current}`, { cache: "no-store" });
        if (!res.ok) throw new Error(String(res.status));
        const next = (await res.json()) as Flow[];
        if (cancelled) return;
        setError(false);
        if (next.length === 0) return;
        lastId.current = next[0]!.id;
        setFresh(new Set(next.map((f) => f.id)));
        setFlows((current) => [...next, ...current].slice(0, MAX_ROWS));
      } catch {
        if (!cancelled) setError(true);
      }
    };
    const timer = setInterval(tick, POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, []);

  if (flows.length === 0) {
    return (
      <p className="rounded-sm border border-dashed border-line-strong px-5 py-8 text-sm text-ink-2">
        No flows ingested yet. Rows appear here within seconds of settlement once the ingestor
        reaches the chain head.
      </p>
    );
  }

  return (
    <div>
      <ol className="flex flex-col" aria-live="polite">
        {flows.map((flow) => (
          <li
            key={flow.id}
            className={`grid grid-cols-[4.5rem_1fr_auto] items-baseline gap-x-3 border-b border-line py-2 ${
              fresh.has(flow.id) ? "row-enter" : ""
            }`}
          >
            <span className="font-mono text-xs text-ink-3">{utcTime(flow.block_time)}</span>
            <Link
              href={`/flows/${flow.id}`}
              className="truncate text-sm underline-offset-2 hover:text-accent-ink hover:underline"
              title={flow.narrative}
            >
              <span className="font-medium">{kindLabel(flow.kind)}</span>{" "}
              <span className="text-ink-2">{flow.narrative.split(": ")[1] ?? ""}</span>
            </Link>
            <span className="font-mono text-sm">
              {flow.visibility === "shielded" ? "Shielded" : usd(flow.usd_value)}{" "}
              <span className="text-xs text-ink-3">{flow.asset}</span>
            </span>
          </li>
        ))}
      </ol>
      {error ? (
        <p role="status" className="mt-2 text-xs text-ink-2">
          Live updates paused: the API is not responding. Retrying every few seconds.
        </p>
      ) : null}
    </div>
  );
}
