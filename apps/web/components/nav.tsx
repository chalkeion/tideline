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
    <nav aria-label="Primary" className="flex flex-1 items-center gap-1 overflow-x-auto text-sm">
      {NAV.map((item) => {
        const current = item.href === "/" ? pathname === "/" : pathname.startsWith(item.href);
        return (
          <Link
            key={item.href}
            href={item.href}
            aria-current={current ? "page" : undefined}
            className={`whitespace-nowrap rounded-sm px-2.5 py-1.5 transition-colors hover:bg-accent-wash hover:text-accent-ink ${
              current ? "bg-accent-wash font-medium text-accent-ink" : "text-ink-2"
            }`}
          >
            {item.label}
          </Link>
        );
      })}
    </nav>
  );
}
