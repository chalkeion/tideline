import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import Link from "next/link";
import { Logo } from "@/components/logo";
import { Nav } from "@/components/nav";
import { getNetwork } from "@/lib/api";
import "./globals.css";

const geistSans = Geist({ variable: "--font-geist-sans", subsets: ["latin"] });
const geistMono = Geist_Mono({ variable: "--font-geist-mono", subsets: ["latin"] });

/** Absolute base for link previews: SITE_URL, else Vercel's production domain. */
const siteUrl =
  process.env.SITE_URL ??
  (process.env.VERCEL_PROJECT_PRODUCTION_URL
    ? `https://${process.env.VERCEL_PROJECT_PRODUCTION_URL}`
    : "http://localhost:3000");

const description = "Every flow on Arc, attributed to who moved it, with the evidence.";

export const metadata: Metadata = {
  metadataBase: new URL(siteUrl),
  title: { default: "Tideline", template: "%s | Tideline" },
  description,
  openGraph: { type: "website", siteName: "Tideline", title: "Tideline", description },
  twitter: { card: "summary_large_image", title: "Tideline", description },
};

export default async function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  // The header must render even when the API is down, so error pages keep their chrome.
  const network = await getNetwork().catch(() => null);
  return (
    <html lang="en" className={`${geistSans.variable} ${geistMono.variable} antialiased`}>
      <body className="min-h-[100dvh] overflow-x-clip font-sans">
        <a
          href="#main"
          className="sr-only focus:not-sr-only focus:absolute focus:left-4 focus:top-3 focus:z-30 focus:rounded-sm focus:bg-ink focus:px-3 focus:py-1.5 focus:text-sm focus:text-page"
        >
          Skip to content
        </a>
        {/* A floating glass island: the only blurred layer, and it is sticky. */}
        <header className="sticky top-3 z-20 px-3 md:px-6">
          <div className="mx-auto flex max-w-[1400px] flex-wrap items-center gap-x-5 gap-y-2 rounded-[1.75rem] border border-line-strong bg-page/70 px-3 py-2 shadow-[0_20px_50px_-30px_var(--shadow)] backdrop-blur-xl md:flex-nowrap md:rounded-full md:pl-4">
            <Link
              href="/"
              aria-label="Tideline home"
              className="flex items-center gap-3 rounded-full transition-opacity duration-500 ease-tide hover:opacity-80"
            >
              <Logo />
              {/* Every figure comes from one network; say which next to the name. */}
              {network ? (
                <span className="hidden items-center gap-1.5 rounded-full border border-line-strong px-2.5 py-1 text-[11px] leading-none text-ink-2 sm:inline-flex">
                  <span className="live-dot" aria-hidden="true" />
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
                placeholder="Search address, entity or token"
                className="h-9 w-full rounded-full border border-line-strong bg-surface/60 px-4 font-mono text-xs transition-[border-color,box-shadow] duration-500 ease-tide placeholder:text-ink-3 focus:border-accent focus:shadow-[0_0_0_4px_var(--accent-wash)] focus:outline-none md:w-72"
              />
            </form>
          </div>
        </header>
        <main id="main" className="stagger mx-auto flex max-w-[1400px] flex-col gap-12 px-4 pb-16 pt-12 md:px-8 md:pt-16">
          {children}
        </main>
        <footer className="mx-auto max-w-[1400px] px-4 pb-10 md:px-8">
          <div className="flex flex-col gap-3 border-t border-line pt-6 text-xs text-ink-3 md:flex-row md:items-center md:justify-between">
            <span className="flex items-center gap-2.5 text-ink-2">
              <Logo />
            </span>
            <p>
              Attributed flow intelligence for Arc{network ? `, reading ${network.name} (chain ${network.chain_id})` : ""}.
              Every attribution carries its evidence.
            </p>
          </div>
        </footer>
      </body>
    </html>
  );
}
