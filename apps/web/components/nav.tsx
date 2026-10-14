"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

const NAV = [
  { href: "/", label: "Overview" },
  { href: "/flows", label: "Flows" },
  { href: "/assets", label: "Assets" },
  { href: "/entities", label: "Entities" },
  { href: "/coverage", label: "Coverage" },
  { href: "/license", label: "Data license" },
];

/** Primary navigation, with the section being viewed marked as current. */
export function Nav() {
  const pathname = usePathname();
  return (
    <nav
      aria-label="Primary"
      className="order-last flex w-full min-w-0 items-center gap-0.5 overflow-x-auto text-[13px] [scrollbar-width:none] md:order-none md:w-auto md:flex-1 [&::-webkit-scrollbar]:hidden"
    >
      {NAV.map((item) => {
        const current = item.href === "/" ? pathname === "/" : pathname.startsWith(item.href);
        return (
          <Link
            key={item.href}
            href={item.href}
            aria-current={current ? "page" : undefined}
            className={`whitespace-nowrap rounded-full px-3.5 py-1.5 transition-[color,background-color] duration-500 ease-tide active:scale-[0.98] ${
              current
                ? "bg-ink/[0.07] font-medium text-ink shadow-[inset_0_0_0_1px_var(--line-strong)]"
                : "text-ink-2 hover:bg-ink/[0.04] hover:text-ink"
            }`}
          >
            {item.label}
          </Link>
        );
      })}
    </nav>
  );
}
