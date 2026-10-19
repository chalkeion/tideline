import "server-only";
import { cache } from "react";
import { TidelineClient } from "@tideline/sdk";

export const API_URL = process.env.TIDELINE_API_URL ?? "http://localhost:8080";

/**
 * Server-side client. The license key never reaches the browser. It sets no fetch cache mode:
 * pages that export `revalidate` cache their upstream calls for that long, and everything else
 * (searchParams pages, route handlers) is uncached by default.
 */
export const api = new TidelineClient({
  baseUrl: API_URL,
  apiKey: process.env.TIDELINE_API_KEY || undefined,
});

/** The Arc network the service indexes, fetched once per request. */
export const getNetwork = cache(() => api.network());

const WINDOWS = [1, 24, 168] as const;

/** Parse a `?window=` search param into a supported window, defaulting to 24h. */
export function parseWindow(value: string | string[] | undefined): number {
  const hours = Number(Array.isArray(value) ? value[0] : value);
  return (WINDOWS as readonly number[]).includes(hours) ? hours : 24;
}
