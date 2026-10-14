"use client";

export default function ErrorPage({ error, reset }: { error: Error; reset: () => void }) {
  return (
    <div className="max-w-[65ch] rounded-sm border border-line-strong px-5 py-8">
      <h1 className="text-lg font-semibold">Tideline could not load this view</h1>
      <p className="mt-2 text-sm leading-relaxed text-ink-2">
        The web app could not reach the Tideline API, or the API returned an error. Check that
        the stream service is running and that TIDELINE_API_URL points at it.
      </p>
      {error.message ? (
        <p className="mt-3 font-mono text-xs text-ink-3">{error.message}</p>
      ) : null}
      <button
        type="button"
        onClick={reset}
        className="mt-5 rounded-sm bg-ink px-3 py-1.5 text-sm font-medium text-page active:translate-y-px"
      >
        Retry
      </button>
    </div>
  );
}
