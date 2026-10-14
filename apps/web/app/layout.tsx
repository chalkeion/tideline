import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import Link from "next/link";
import { Nav } from "@/components/nav";
import { getNetwork } from "@/lib/api";
import "./globals.css";

const geistSans = Geist({ variable: "--font-geist-sans", subsets: ["latin"] });
const geistMono = Geist_Mono({ variable: "--font-geist-mono", subsets: ["latin"] });

export const metadata: Metadata = {
  title: { default: "Tideline", template: "%s | Tideline" },
  description: "Attributed stablecoin-flow intelligence for Arc.",
};

export default async function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  // The header must render even when the API is down, so error pages keep their chrome.
  const network = await getNetwork().catch(() => null);
  return (
    <html lang="en" className={`${geistSans.variable} ${geistMono.variable} antialiased`}>
      <body className="min-h-[100dvh] font-sans">
        <a
          href="#main"
          className="sr-only focus:not-sr-only focus:absolute focus:left-4 focus:top-3 focus:z-30 focus:rounded-sm focus:bg-ink focus:px-3 focus:py-1.5 focus:text-sm focus:text-page"
        >
          Skip to content
        </a>
        <header className="sticky top-0 z-20 border-b border-line bg-page">
          <div className="mx-auto flex max-w-[1400px] flex-wrap items-center gap-x-6 gap-y-2 px-4 py-2 md:h-14 md:flex-nowrap md:px-8 md:py-0">
            <Link href="/" className="flex items-baseline gap-2 text-base font-semibold tracking-tight">
              Tideline
              {/* Every figure comes from one network; say which next to the name. */}
              {network ? (
                <span className="rounded-sm border border-line-strong px-1.5 py-0.5 text-[11px] font-normal leading-none tracking-normal text-ink-2">
                  {network.name}
                </span>
              ) : null}
            </Link>
            <Nav />
            <form action="/search" role="search" className="w-full md:w-auto">
              <label htmlFor="q" className="sr-only">
                Search by address or entity
              </label>
              <input
                id="q"
                name="q"
                type="search"
                required
                placeholder="Address or entity"
                className="h-8 w-full rounded-sm border border-line-strong bg-surface px-2.5 font-mono text-xs placeholder:text-ink-3 md:w-64"
              />
            </form>
          </div>
        </header>
        <main id="main" className="mx-auto flex max-w-[1400px] flex-col gap-10 px-4 py-8 md:px-8">
          {children}
        </main>
      </body>
    </html>
  );
}
