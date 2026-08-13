import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useQuery } from "@tanstack/react-query";
import { ArrowLeft, ChevronDown, CornerDownRight, History } from "lucide-react";
import { fileHistory, getCommitDiff, type CommitSummary } from "@/lib/api/history";
import { getPage, type PageResponse } from "@/lib/api/docs";

/** TOC entry derived from rendered h1-h3 headings. */
export interface TocEntry {
	id: string;
	index: number;
	text: string;
	level: number;
}

const headingSelector = "h1, h2, h3";

/** Goldmark/CommonMark can render leading YAML front matter as `<hr>` followed
 * by one large setext `<h2>`. Keep that rendering artifact out of the TOC. */
function isRenderedFrontMatterHeading(
	root: HTMLElement,
	heading: Element,
): boolean {
	if (heading.tagName !== "H2") return false;
	const delimiter = heading.previousElementSibling;
	if (delimiter?.tagName !== "HR" || delimiter !== root.firstElementChild) {
		return false;
	}

	const text = (heading.textContent ?? "").trim();
	const yamlKeys = text.match(/(?:^|\n)\s*[A-Za-z_][\w.-]*\s*:/g) ?? [];
	return yamlKeys.length >= 2;
}

function formatDate(iso: string): string {
	return new Date(iso).toLocaleDateString("zh-CN", {
		year: "numeric",
		month: "2-digit",
		day: "2-digit",
	});
}

function formatDateTime(iso: string): string {
	return new Date(iso).toLocaleString("zh-CN", {
		month: "2-digit",
		day: "2-digit",
		hour: "2-digit",
		minute: "2-digit",
	});
}

/** Find the heading element for a TOC entry by index (ids can be reset by re-render). */
function headingEl(e: TocEntry): HTMLElement | null {
	return (
		(document.querySelectorAll(headingSelector)[e.index] as HTMLElement) ?? null
	);
}

export function extractToc(root: HTMLElement): TocEntry[] {
	const out: TocEntry[] = [];
	root.querySelectorAll(headingSelector).forEach((el, i) => {
		if (isRenderedFrontMatterHeading(root, el)) return;
		const level = Number(el.tagName[1]);
		const text = (el.textContent ?? "").trim();
		if (!text) return;
		const id = `toc-${i}`;
		el.id = id;
		out.push({ id, index: i, text, level });
	});
	return out;
}

/** Sidebar TOC with click-to-scroll, URL hash anchors and active-heading highlight. */
export function TocPanel({ entries }: { entries: TocEntry[] }) {
	const [active, setActive] = useState<string | null>(null);
	const [open, setOpen] = useState(true);

	const scrollToHash = () => {
		const m = window.location.hash.match(/^#toc-(\d+)$/);
		if (!m) return;
		const idx = Number(m[1]);
		const el = document.querySelectorAll(headingSelector)[idx] as
			| HTMLElement
			| undefined;
		el?.scrollIntoView({ behavior: "auto", block: "start" });
	};

	useEffect(() => {
		if (entries.length === 0) return;
		const onScroll = () => {
			let current: string | null = null;
			for (const e of entries) {
				const el = headingEl(e);
				if (el && el.getBoundingClientRect().top <= 90) current = e.id;
			}
			setActive(current);
		};
		window.addEventListener("scroll", onScroll, { passive: true });
		window.addEventListener("hashchange", scrollToHash);
		onScroll();
		scrollToHash();
		return () => {
			window.removeEventListener("scroll", onScroll);
			window.removeEventListener("hashchange", scrollToHash);
		};
	}, [entries]);

	if (entries.length === 0) return null;
	return (
		<nav aria-label="目录" className="space-y-0.5 p-2">
			<button
				type="button"
				onClick={() => setOpen((v) => !v)}
				className="mono-label flex w-full items-center gap-1 px-2 pb-1 text-[var(--color-ink-3)] hover:text-[var(--color-ink)]"
			>
				<ChevronDown
					className={`size-3 shrink-0 transition-transform ${open ? "" : "-rotate-90"}`}
				/>
				toc
			</button>
			{open &&
				entries.map((e) => (
					<button
						key={e.id}
						type="button"
						onClick={() => {
							headingEl(e)?.scrollIntoView({
								behavior: "smooth",
								block: "start",
							});
							window.history.pushState(null, "", `#toc-${e.index}`);
						}}
						className={`block w-full truncate rounded-sm px-2 py-1 text-left text-xs hover:bg-[var(--color-surface-accent)] ${
							active === e.id
								? "text-[var(--color-accent)]"
								: "text-[var(--color-ink-2)]"
						}`}
						style={{ paddingLeft: `${8 + (e.level - 1) * 10}px` }}
						title={e.text}
					>
						{e.text}
					</button>
				))}
		</nav>
	);
}

/** Version history list (shown under the TOC); clicking loads that revision. */
export function VersionPanel({
	projectId,
	filePath,
	currentVersion,
	onSelect,
}: {
	projectId: string;
	filePath: string;
	currentVersion: string | null; // null = latest
	onSelect: (sha: string | null) => void;
}) {
	const { data } = useQuery({
		queryKey: ["history", projectId, filePath],
		queryFn: () => fileHistory(projectId, filePath),
		enabled: filePath.length > 0,
	});
	const [open, setOpen] = useState(true);
	// Hovered commit + its anchor rect, rendered via a portal to body so the
	// tooltip escapes the sidebar's stacking context (aside is z-30) and is
	// never clipped by overflow or covered by page-level floating layers.
	const [hoverSha, setHoverSha] = useState<string | null>(null);
	const [hoverRect, setHoverRect] = useState<DOMRect | null>(null);
	const btnRefs = useRef<Map<string, HTMLButtonElement>>(new Map());
	if (!data || data.commits.length === 0) return null;
	return (
		<div className="space-y-0.5 p-2">
			<button
				type="button"
				onClick={() => setOpen((v) => !v)}
				className="mono-label flex w-full items-center gap-1 px-2 pb-1 text-[var(--color-ink-3)] hover:text-[var(--color-ink)]"
			>
				<ChevronDown
					className={`size-3 shrink-0 transition-transform ${open ? "" : "-rotate-90"}`}
				/>
				versions · {data.commits.length}
			</button>
			{open && (
				<>
					<button
						type="button"
						onClick={() => onSelect(null)}
						className={`flex w-full items-center gap-1.5 rounded-sm px-2 py-1 text-left text-xs hover:bg-[var(--color-surface-accent)] ${
							currentVersion === null
								? "text-[var(--color-accent)]"
								: "text-[var(--color-ink-2)]"
						}`}
					>
						<CornerDownRight className="size-3 shrink-0" />
						<span className="truncate">最新版本</span>
					</button>
					{data.commits.map((c: CommitSummary) => (
						<button
							key={c.sha}
							type="button"
							onClick={() => onSelect(c.sha)}
							onMouseEnter={() => {
								const el = btnRefs.current.get(c.sha);
								if (el) setHoverRect(el.getBoundingClientRect());
								setHoverSha(c.sha);
							}}
							onMouseLeave={() => {
								setHoverSha(null);
								setHoverRect(null);
							}}
							ref={(el) => {
								if (el) btnRefs.current.set(c.sha, el);
								else btnRefs.current.delete(c.sha);
							}}
							className={`flex w-full items-center gap-1.5 rounded-sm px-2 py-1 text-left text-xs hover:bg-[var(--color-surface-accent)] ${
								currentVersion === c.sha
									? "text-[var(--color-accent)]"
									: "text-[var(--color-ink-2)]"
							}`}
						>
							<History className="size-3 shrink-0" />
							<span className="truncate">
								{c.sha.slice(0, 7)} · {c.message}
							</span>
						</button>
					))}
				</>
			)}
			{hoverSha &&
				hoverRect &&
				createPortal(
					<div
						className="pointer-events-none fixed z-[100] w-64 rounded-[var(--radius)] border border-[var(--color-rule)] bg-[var(--color-paper)] p-3 shadow-lg"
						style={{
							left: hoverRect.right + 8,
							top: hoverRect.top + hoverRect.height / 2,
							transform: "translateY(-50%)",
						}}
					>
						<p className="break-words text-sm leading-snug text-[var(--color-ink)]">
							{data.commits.find((c) => c.sha === hoverSha)?.message}
						</p>
						<p className="mono-label mt-2 text-[11px] text-[var(--color-ink-3)]">
							{data.commits.find((c) => c.sha === hoverSha)?.author} ·{" "}
							{formatDate(data.commits.find((c) => c.sha === hoverSha)?.date ?? "")}
						</p>
					</div>,
					document.body,
				)}
		</div>
	);
}

/** Right-side history drawer: per-commit author/date/full sha plus a lazy
 * per-file diff (numstat) expanded on demand. */
export function HistoryPanel({
	projectId,
	filePath,
	currentVersion,
	onSelect,
	onClose,
}: {
	projectId: string;
	filePath: string;
	currentVersion: string | null; // null = latest
	onSelect: (sha: string | null) => void;
	onClose: () => void;
}) {
	// Same queryKey as VersionPanel, so the commit list is fetched once.
	const { data } = useQuery({
		queryKey: ["history", projectId, filePath],
		queryFn: () => fileHistory(projectId, filePath),
		enabled: filePath.length > 0,
	});
	const [openSha, setOpenSha] = useState<string | null>(null);
	if (!data || data.commits.length === 0) return null;
	return (
		<div className="flex h-full flex-col">
			<div className="mono-label flex items-center justify-between gap-2 border-b border-[var(--color-rule)] px-4 py-3">
				<span className="truncate text-[var(--color-ink-3)]">
					history · {filePath}
				</span>
				<button
					type="button"
					onClick={onClose}
					aria-label="关闭历史"
					className="shrink-0 text-[var(--color-ink-3)] hover:text-[var(--color-ink)]"
				>
					✕
				</button>
			</div>
			<div className="scrollbar-hidden min-h-0 flex-1 overflow-y-auto p-2">
				<button
					type="button"
					onClick={() => onSelect(null)}
					className={`flex w-full items-center gap-1.5 rounded-sm px-2 py-1.5 text-left text-xs hover:bg-[var(--color-surface-accent)] ${
						currentVersion === null
							? "text-[var(--color-accent)]"
							: "text-[var(--color-ink-2)]"
					}`}
				>
					<CornerDownRight className="size-3 shrink-0" />
					<span className="truncate">最新版本</span>
				</button>
				{data.commits.map((c) => {
					const active = currentVersion === c.sha;
					const expanded = openSha === c.sha;
					return (
						<div
							key={c.sha}
							className={`rounded-sm ${
								active
									? "bg-[var(--color-surface-accent)]"
									: "hover:bg-[var(--color-surface-accent)]"
							}`}
						>
							<div className="flex items-start gap-1 px-2 py-1.5">
								<button
									type="button"
									aria-label={expanded ? "收起详情" : "展开详情"}
									onClick={() => setOpenSha(expanded ? null : c.sha)}
									className="mt-0.5 shrink-0 text-[var(--color-ink-3)] hover:text-[var(--color-ink)]"
								>
									<ChevronDown
										className={`size-3 transition-transform ${expanded ? "" : "-rotate-90"}`}
									/>
								</button>
								<button
									type="button"
									onClick={() => onSelect(c.sha)}
									title={c.message}
									className="min-w-0 flex-1 text-left"
								>
									<span
										className={`block truncate text-xs ${
											active
												? "text-[var(--color-accent)]"
												: "text-[var(--color-ink)]"
										}`}
									>
										{c.message || "(无提交信息)"}
									</span>
									<span className="block truncate text-[11px] text-[var(--color-ink-3)]">
										{c.author} · {formatDateTime(c.date)}
									</span>
									<span className="block truncate font-mono text-[11px] text-[var(--color-accent)]">
										{c.sha}
									</span>
								</button>
							</div>
							{expanded && <CommitFiles projectId={projectId} sha={c.sha} />}
						</div>
					);
				})}
			</div>
		</div>
	);
}

/** Per-file +N/-N diff stats for one commit (fetched on expand). */
function CommitFiles({ projectId, sha }: { projectId: string; sha: string }) {
	const { data, isLoading } = useQuery({
		queryKey: ["commit-diff", projectId, sha],
		queryFn: () => getCommitDiff(projectId, sha, "numstat"),
	});
	if (isLoading) {
		return (
			<p className="mono-label px-2 pb-2 pl-7 text-[11px] text-[var(--color-ink-3)]">
				loading…
			</p>
		);
	}
	const stats = data?.stats ?? [];
	const added = stats.reduce((n, s) => n + s.added, 0);
	const deleted = stats.reduce((n, s) => n + s.deleted, 0);
	return (
		<div className="mb-2 space-y-0.5 pl-7 pr-2">
			<p className="mono-label text-[11px] text-[var(--color-ink-3)]">
				+{added} -{deleted} · {stats.length} files
			</p>
			{stats.map((s) => (
				<p
					key={s.path}
					className="truncate font-mono text-[11px] text-[var(--color-ink-2)]"
					title={s.path}
				>
					<span className="text-[var(--color-success)]">+{s.added}</span>{" "}
					<span className="text-[var(--color-destructive)]">-{s.deleted}</span>{" "}
					{s.path}
				</p>
			))}
		</div>
	);
}

/** Loads a page (optionally at a historical revision) and extracts its TOC. */
export function useVersionedPage(
	projectId: string,
	filePath: string,
	atSha: string | null,
) {
	const query = useQuery({
		queryKey: ["docs", "page", projectId, filePath, atSha ?? "latest"],
		queryFn: (): Promise<PageResponse> =>
			getPage(projectId, filePath, "html", atSha ?? undefined),
		enabled: filePath.length > 0,
	});
	return query;
}

export { ArrowLeft };
