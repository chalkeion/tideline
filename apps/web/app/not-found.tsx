import Link from "next/link";

export default function NotFound() {
  return (
    <div className="max-w-[65ch] rounded-sm border border-line-strong px-5 py-8">
      <h1 className="text-lg font-semibold">Nothing on file</h1>
      <p className="mt-2 text-sm leading-relaxed text-ink-2">
        Tideline has no record matching that address, entity or flow.
      </p>
      <Link prefetch={false} href="/flows" className="mt-4 inline-block text-sm text-accent-ink underline underline-offset-2">
        Browse recent flows
      </Link>
    </div>
  );
}
