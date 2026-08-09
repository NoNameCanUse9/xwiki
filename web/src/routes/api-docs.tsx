import { useEffect, useRef } from "react";
import { Link } from "react-router-dom";
import { ArrowLeft, Moon, Sun } from "lucide-react";
import { useTheme } from "@/lib/theme";
import scalarThemeCss from "./api-docs.css?inline";

export default function ApiDocsPage() {
	const containerRef = useRef<HTMLDivElement>(null);
	const { theme, toggle } = useTheme();

	// Scalar ships a Vue component; in a React tree we must mount it through
	// createApiReference (HTML API), which creates its own Vue app instance on
	// the target element. Mounting the Vue component directly as a React
	// element crashes React with error #130 (object child).
	useEffect(() => {
		let instance: ReturnType<
			typeof import("@scalar/api-reference").createApiReference
		> | null = null;
		let cancelled = false;
		void Promise.all([
			import("@scalar/api-reference"),
			import("@scalar/api-reference/style.css"),
		]).then(([m]) => {
			if (cancelled || !containerRef.current) return;
			instance = m.createApiReference(containerRef.current, {
				url: "/api/openapi.json",
				theme: "default",
				layout: "modern",
				showSidebar: true,
				showDeveloperTools: "always",
				darkMode: theme === "dark",
				forceDarkModeState: theme,
				hideDarkModeToggle: true,
				customCss: scalarThemeCss,
			});
		});
		return () => {
			cancelled = true;
			instance?.destroy();
		};
	}, [theme]);

	return (
		<div className="api-docs-shell flex min-h-screen flex-col">
			<header className="api-docs-masthead">
				<Link
					to="/"
					className="api-docs-back"
					aria-label="Back to workspace"
				>
					<ArrowLeft className="size-3.5" aria-hidden="true" />
					<span>workspace</span>
				</Link>
				<div className="api-docs-identity" aria-label="API reference">
					<span className="api-docs-identity-label">reference</span>
					<span className="api-docs-identity-meta">api · openapi 3.0.3</span>
				</div>
				<div className="api-docs-controls">
					<button
						type="button"
						className="api-docs-theme-toggle"
						onClick={toggle}
						aria-label={
							theme === "dark" ? "切换到亮色模式" : "切换到暗色模式"
						}
						title={theme === "dark" ? "切换到亮色模式" : "切换到暗色模式"}
					>
						{theme === "dark" ? (
							<Sun className="size-4" aria-hidden="true" />
						) : (
							<Moon className="size-4" aria-hidden="true" />
						)}
					</button>
				</div>
			</header>
			<main className="api-docs-main">
				<div
					ref={containerRef}
					className="api-docs-reference"
					aria-label="API reference"
				/>
			</main>
		</div>
	);
}
