import { describe, expect, it } from "vitest";
import { isEvmAddress, ms, rateLabel, shortAddress, spanLabel, usd, utc, windowLabel } from "./format";

describe("format", () => {
  it("formats USD across magnitudes and marks shielded amounts", () => {
    expect(usd(12_000_000)).toBe("$12.00M");
    expect(usd(45_300)).toBe("$45.3K");
    expect(usd(1.02)).toBe("$1.02");
    expect(usd(null)).toBe("Shielded");
  });

  it("formats latency in ms or seconds", () => {
    expect(ms(820)).toBe("820 ms");
    expect(ms(2450)).toBe("2.45 s");
    expect(ms(null)).toBe("n/a");
  });

  it("shortens addresses but leaves short strings alone", () => {
    expect(shortAddress("0x631fdaafe9ff675bf7a122013d1555513363dcc7")).toBe("0x631fda…63dcc7");
    expect(shortAddress("0x0")).toBe("0x0");
  });

  it("renders absolute UTC time", () => {
    expect(utc("2026-10-07T22:39:01.250Z")).toBe("2026-10-07 22:39:01 UTC");
  });

  it("labels windows", () => {
    expect(windowLabel(1)).toBe("1h");
    expect(windowLabel(24)).toBe("24h");
    expect(windowLabel(168)).toBe("7d");
  });

  it("states a span in the largest honest unit", () => {
    expect(spanLabel(20_000)).toBe("1 minute");
    expect(spanLabel(21 * 60_000)).toBe("21 minutes");
    expect(spanLabel(5 * 3_600_000 + 59 * 60_000)).toBe("5 hours");
    expect(spanLabel(3 * 86_400_000)).toBe("3 days");
  });

  it("labels a reference rate with where it came from", () => {
    expect(rateLabel(1.1186, "ECB euro reference rate", "2026-10-08")).toBe(
      "1.1186 USD (ECB euro reference rate, 2026-10-08)",
    );
    expect(rateLabel(1.08, "configured static rate", null)).toBe("1.0800 USD (configured static rate)");
  });

  it("recognises EVM addresses only", () => {
    expect(isEvmAddress("0x631fdaafe9ff675bf7a122013d1555513363dcc7")).toBe(true);
    expect(isEvmAddress("0x1234")).toBe(false);
  });
});
