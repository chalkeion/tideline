"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import type { TimeBucket } from "@tideline/sdk";
import { ASSETS, usd } from "@/lib/format";

const SERIES: Record<string, string> = {
  USDC: "var(--series-usdc)",
  EURC: "var(--series-eurc)",
  USYC: "var(--series-usyc)",
};

const HEIGHT = 220;
const PAD = { top: 12, right: 8, bottom: 24, left: 56 };
const GAP = 2;

/** Largest "nice" step giving about four gridlines under `max`. */
function niceStep(max: number): number {
  const raw = max / 4;
  const pow = 10 ** Math.floor(Math.log10(raw));
  const unit = raw / pow;
  return (unit >= 5 ? 5 : unit >= 2 ? 2 : 1) * pow;
}

function tickTime(iso: string, bucketMinutes: number): string {
  const d = new Date(iso).toISOString();
  return bucketMinutes >= 1440 ? d.slice(5, 10) : d.slice(11, 16);
}

/** Stacked columns of notional USD volume per time bucket, one colour per asset. */
export function VolumeChart({
  series,
  bucketMinutes,
}: {
  series: TimeBucket[];
  bucketMinutes: number;
}) {
  const wrap = useRef<HTMLDivElement>(null);
  // Unknown until measured: rendering at a guessed width would force the layout wider.
  const [width, setWidth] = useState<number | null>(null);
  const [hover, setHover] = useState<number | null>(null);

  useEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry) setWidth(Math.max(240, Math.floor(entry.contentRect.width)));
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const buckets = useMemo(() => {
    const byTime = new Map<string, Record<string, number>>();
    for (const point of series) {
      const row = byTime.get(point.t) ?? {};
      row[point.asset] = point.volume_usd;
      byTime.set(point.t, row);
    }
    return [...byTime.entries()]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([t, values]) => ({
        t,
        values,
        total: Object.values(values).reduce((sum, v) => sum + v, 0),
      }));
  }, [series]);

  const assets = ASSETS.filter((asset) => buckets.some((b) => (b.values[asset] ?? 0) > 0));
  const max = Math.max(...buckets.map((b) => b.total), 0);

  if (buckets.length === 0 || max === 0) {
    return (
      <p className="rounded-sm border border-dashed border-line-strong px-5 py-8 text-sm text-ink-2">
        No volume in this window yet.
      </p>
    );
  }

  if (width === null) {
    return <div ref={wrap} className="w-full" style={{ height: HEIGHT + 24 }} />;
  }

  const step = niceStep(max);
  const top = Math.ceil(max / step) * step;
  const plotW = width - PAD.left - PAD.right;
  const plotH = HEIGHT - PAD.top - PAD.bottom;
  const slot = plotW / buckets.length;
  const barW = Math.max(2, Math.min(24, slot - GAP));
  const y = (v: number) => PAD.top + plotH - (v / top) * plotH;
  const ticks = Array.from({ length: Math.round(top / step) + 1 }, (_, i) => i * step);
  const labelEvery = Math.max(1, Math.ceil(buckets.length / Math.max(2, Math.floor(plotW / 64))));
  const active = hover === null ? null : buckets[hover];

  return (
    <div ref={wrap} className="relative w-full overflow-hidden">
      {assets.length > 1 ? (
        <ul className="mb-2 flex gap-4 text-xs text-ink-2">
          {assets.map((asset) => (
            <li key={asset} className="flex items-center gap-1.5">
              <span className="size-2.5 rounded-sm" style={{ background: SERIES[asset] }} />
              {asset}
            </li>
          ))}
        </ul>
      ) : null}
      <svg
        width={width}
        height={HEIGHT}
        role="img"
        aria-label={`Notional USD volume per ${bucketMinutes} minute bucket, stacked by asset. The asset table beside this chart carries the same totals.`}
        onMouseLeave={() => setHover(null)}
      >
        {ticks.map((tick) => (
          <g key={tick}>
            <line
              x1={PAD.left}
              x2={width - PAD.right}
              y1={y(tick)}
              y2={y(tick)}
              stroke={tick === 0 ? "var(--line-strong)" : "var(--line)"}
            />
            <text
              x={PAD.left - 8}
              y={y(tick)}
              textAnchor="end"
              dominantBaseline="middle"
              className="fill-ink-3 font-mono text-[10px]"
            >
              {usd(tick).replace(/\.0+(?=[KMB]?$)/, "")}
            </text>
          </g>
        ))}
        {buckets.map((bucket, i) => {
          const x = PAD.left + i * slot + (slot - barW) / 2;
          let acc = 0;
          const present = assets.filter((asset) => (bucket.values[asset] ?? 0) > 0);
          return (
            <g key={bucket.t} opacity={hover === null || hover === i ? 1 : 0.45}>
              {present.map((asset, index) => {
                const value = bucket.values[asset] ?? 0;
                const y1 = y(acc + value);
                const h = Math.max(1, y(acc) - y1 - (index > 0 ? GAP : 0));
                acc += value;
                const isTop = index === present.length - 1;
                const r = isTop ? Math.min(4, barW / 2, h) : 0;
                // Rounded data-end, square at the baseline.
                const d = `M${x},${y1 + h} V${y1 + r} Q${x},${y1} ${x + r},${y1} H${x + barW - r} Q${x + barW},${y1} ${x + barW},${y1 + r} V${y1 + h} Z`;
                return <path key={asset} d={d} fill={SERIES[asset]} />;
              })}
              {i % labelEvery === 0 ? (
                <text
                  x={x + barW / 2}
                  y={HEIGHT - 6}
                  textAnchor="middle"
                  className="fill-ink-3 font-mono text-[10px]"
                >
                  {tickTime(bucket.t, bucketMinutes)}
                </text>
              ) : null}
              <rect
                x={PAD.left + i * slot}
                y={PAD.top}
                width={slot}
                height={plotH}
                fill="transparent"
                onMouseEnter={() => setHover(i)}
              />
            </g>
          );
        })}
      </svg>
      {active && hover !== null ? (
        <div
          role="status"
          className="pointer-events-none absolute top-8 z-10 min-w-36 rounded-sm border border-line-strong bg-surface px-3 py-2 text-xs shadow-sm"
          style={
            hover > buckets.length / 2
              ? { right: width - (PAD.left + hover * slot) + 4 }
              : { left: PAD.left + (hover + 1) * slot + 4 }
          }
        >
          <p className="font-mono text-ink-2">
            {new Date(active.t).toISOString().slice(0, 16).replace("T", " ")} UTC
          </p>
          {assets.map((asset) => (
            <p key={asset} className="mt-1 flex items-center justify-between gap-4">
              <span className="flex items-center gap-1.5">
                <span className="size-2 rounded-sm" style={{ background: SERIES[asset] }} />
                {asset}
              </span>
              <span className="font-mono">{usd(active.values[asset] ?? 0)}</span>
            </p>
          ))}
        </div>
      ) : null}
    </div>
  );
}
