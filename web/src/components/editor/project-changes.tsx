import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useQuery } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { listCommits, type CommitSummary } from "@/lib/api/history";

function formatDateTime(iso: string): string {
	return new Date(iso).toLocaleString("zh-CN", {
		month: "2-digit",
		day: "2-digit",
		hour: "2-digit",
		minute: "2-digit",
	});
}

function CommitNode({
	commit,
	side,
}: {
	commit: CommitSummary;
	side: "top" | "bottom";
}) {
	const message = commit.message || "（无提交信息）";
	const nodeRef = useRef<HTMLSpanElement>(null);
	const [hoverRect, setHoverRect] = useState<DOMRect | null>(null);

	const showTooltip = () => {
		const rect = nodeRef.current?.getBoundingClientRect();
		if (rect) setHoverRect(rect);
	};
	const hideTooltip = () => setHoverRect(null);
	const tooltip = hoverRect
		? (() => {
			const width = 256;
			const margin = 8;
			const above = side === "top" || hoverRect.bottom + 180 > window.innerHeight - margin;
			const left = Math.min(
				Math.max(hoverRect.left + hoverRect.width / 2, width / 2 + margin),
				window.innerWidth - width / 2 - margin,
			);
			const top = above ? hoverRect.top - margin : hoverRect.bottom + margin;
			return createPortal(
				<div
					id={`commit-tooltip-${commit.sha}`}
					role="tooltip"
					className="pointer-events-none fixed z-[100] w-64 rounded-[var(--radius)] border border-[var(--color-rule)] bg-[var(--color-paper)] p-3 text-left shadow-xl"
					style={{ left, top, transform: `translate(-50%, ${above ? "-100%" : "0"})` }}
				>
					<p className="break-words text-sm font-medium leading-snug text-[var(--color-ink)]">{message}</p>
					<p className="mt-2 text-xs text-[var(--color-ink-2)]">{commit.author} · {formatDateTime(commit.date)}</p>
					<code className="mt-1 block text-[11px] text-[var(--color-accent)]">{commit.sha.slice(0, 12)}</code>
				</div>,
				document.body,
			);
		  })()
		: null;

	return (
		<div
			className="group relative h-24 min-w-32 flex-1"
			data-side={side}
			onMouseEnter={showTooltip}
			onMouseLeave={hideTooltip}
		>
			<span
				className={`absolute left-1/2 max-w-40 -translate-x-1/2 truncate text-center text-[11px] text-[var(--color-ink-2)] ${side === "top" ? "top-0" : "bottom-0"}`}
			>
				{message}
			</span>
			<span
				ref={nodeRef}
				data-track-marker="true"
				data-track-anchor="center"
				role="img"
				tabIndex={0}
				aria-label={`提交：${message}`}
				aria-describedby={hoverRect ? `commit-tooltip-${commit.sha}` : undefined}
				data-side={side}
				onFocus={showTooltip}
				onBlur={hideTooltip}
				className="absolute left-1/2 top-1/2 z-10 flex size-5 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full outline-none"
			>
				<span className="size-3 rounded-full border-2 border-[var(--color-accent)] bg-[var(--color-paper)] transition-shadow group-hover:ring-4 group-hover:ring-[var(--color-surface-accent)] group-focus-within:ring-4 group-focus-within:ring-[var(--color-surface-accent)]" />
			</span>
			<span
				data-track-tick="true"
				data-track-anchor="center"
				aria-hidden="true"
				className={`absolute left-1/2 h-1.5 w-px -translate-x-1/2 bg-[var(--color-rule)] ${side === "top" ? "top-[calc(50%-0.375rem)]" : "top-1/2"}`}
			/>
			{tooltip}
		</div>
	);
}

/** Compact project timeline showing the five latest commits. */
export function ProjectChanges({ projectId }: { projectId: string }) {
	const { data, isLoading, isError } = useQuery({
		queryKey: ["commits", projectId],
		queryFn: () => listCommits(projectId, 5, 0),
		enabled: projectId.length > 0,
	});
	const commits = (data?.commits ?? []).slice(0, 5).reverse();
	const scrollRef = useRef<HTMLDivElement>(null);
	const [canScroll, setCanScroll] = useState(false);
	const [atStart, setAtStart] = useState(true);
	const [atEnd, setAtEnd] = useState(true);
	const updateScrollState = () => {
		const element = scrollRef.current;
		if (!element) return;
		const maxScroll = Math.max(0, element.scrollWidth - element.clientWidth);
		setCanScroll(maxScroll > 1);
		setAtStart(element.scrollLeft <= 1);
		setAtEnd(element.scrollLeft >= maxScroll - 1);
	};
	useEffect(() => {
		updateScrollState();
		window.addEventListener("resize", updateScrollState);
		return () => window.removeEventListener("resize", updateScrollState);
	}, [commits.length]);
	const scrollTimeline = (direction: "left" | "right") => {
		const element = scrollRef.current;
		if (!element) return;
		element.scrollBy({
			left: direction === "right" ? element.clientWidth * 0.8 : -element.clientWidth * 0.8,
			behavior: "smooth",
		});
	};

	return (
		<section className="mt-auto space-y-3 pt-24" aria-label="项目变更记录">
			{isLoading && (
				<p className="py-4 text-center text-sm text-[var(--color-ink-3)]">loading…</p>
			)}
			{isError && (
				<p className="py-4 text-center text-sm text-[var(--color-destructive)]">
					变更记录加载失败
				</p>
			)}
			{data && commits.length === 0 && (
				<p className="py-4 text-center text-sm text-[var(--color-ink-2)]">
					还没有文档变更
				</p>
			)}
			{commits.length > 0 && (
				<div className="relative flex items-center gap-2">
					{canScroll && (
						<button
							type="button"
							aria-label="向左滚动时间线"
							disabled={atStart}
							onClick={() => scrollTimeline("left")}
							className="shrink-0 rounded-full p-1 text-[var(--color-ink-3)] hover:bg-[var(--color-surface-accent)] hover:text-[var(--color-accent)] disabled:opacity-30"
						>
							<ChevronLeft className="size-4" />
						</button>
					)}
					<div
						ref={scrollRef}
						data-timeline-scroll
						className="min-w-0 flex-1 overflow-x-auto pb-1"
						onScroll={updateScrollState}
						role="region"
						aria-label="项目变更时间线"
					>
						<div className="relative min-w-[720px] px-4">
						<div
							aria-hidden="true"
							className="absolute left-[8%] right-[8%] top-1/2 h-px bg-[var(--color-accent)]"
						/>
						<div className="relative flex h-24 items-stretch justify-between gap-4">
							{commits.map((commit, index) => (
								<CommitNode
									key={commit.sha}
									commit={commit}
									side={index % 2 === 0 ? "top" : "bottom"}
								/>
							))}
						</div>
						</div>
					</div>
					{canScroll && (
						<button
							type="button"
							aria-label="向右滚动时间线"
							disabled={atEnd}
							onClick={() => scrollTimeline("right")}
							className="shrink-0 rounded-full p-1 text-[var(--color-ink-3)] hover:bg-[var(--color-surface-accent)] hover:text-[var(--color-accent)] disabled:opacity-30"
						>
							<ChevronRight className="size-4" />
						</button>
					)}
				</div>
			)}
		</section>
	);
}
