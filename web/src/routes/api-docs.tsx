import { useEffect, useRef } from "react";
import { Link } from "react-router-dom";
import { ArrowLeft } from "lucide-react";

export default function ApiDocsPage() {
	const containerRef = useRef<HTMLDivElement>(null);

	// Scalar ships a Vue component; in a React tree we must mount it through
	// createApiReference (HTML API), which creates its own Vue app instance on
	// the target element. Mounting the Vue component directly as a React
	// element crashes React with error #130 (object child).
	useEffect(() => {
		let instance: ReturnType<
			typeof import("@scalar/api-reference").createApiReference
		> | null = null;
		let cancelled = false;
		void import("@scalar/api-reference").then((m) => {
			if (cancelled || !containerRef.current) return;
			instance = m.createApiReference(containerRef.current, {
				url: "/api/openapi.json",
			});
		});
		return () => {
			cancelled = true;
			instance?.destroy();
		};
	}, []);

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
				<div ref={containerRef} className="min-h-[60vh]" />
			</main>
		</div>
	);
}
