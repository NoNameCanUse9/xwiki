import { lazy, Suspense } from "react";
import { Link } from "react-router-dom";
import { ArrowLeft } from "lucide-react";

// Scalar reference is heavy; load it only for this route.
const ApiReference = lazy(() =>
  import("@scalar/api-reference").then((m) => ({
    default: (props: Record<string, unknown>) => (
      // @ts-expect-error web component props
      <m.ApiReference {...props} />
    ),
  })),
);

export default function ApiDocsPage() {
  return (
    <div className="flex min-h-screen flex-col">
      <header className="flex items-center justify-between border-b border-[var(--color-rule)] px-6 py-4">
        <Link
          to="/"
          className="mono-label flex items-center gap-2 text-[var(--color-ink-3)] hover:text-[var(--color-accent)]"
        >
          <ArrowLeft className="size-3.5" />
          workspace
        </Link>
        <span className="mono-label text-[var(--color-ink-3)]">
          api · openapi 3.0
        </span>
      </header>
      <main className="flex-1">
        <Suspense
          fallback={
            <p className="mono-label px-6 py-8 text-[var(--color-ink-3)]">
              loading api reference…
            </p>
          }
        >
          <ApiReference url="/api/openapi.json" />
        </Suspense>
      </main>
    </div>
  );
}
