import { useState } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { ChevronDown, FileText, GitCommitHorizontal } from "lucide-react";
import {
	getCommitDiff,
	listCommits,
	type CommitSummary,
} from "@/lib/api/history";
import { parseDocumentPatch } from "./project-changes-parser";

function formatDateTime(iso: string): string {
	return new Date(iso).toLocaleString("zh-CN", {
		month: "2-digit",
		day: "2-digit",
		hour: "2-digit",
		minute: "2-digit",
	});
}

function ChangeDetails({
	projectId,
	sha,
	onOpen,
}: {
	projectId: string;
	sha: string;
	onOpen: (path: string) => void;
}) {
	const { data, isLoading, isError } = useQuery({
		queryKey: ["commit-diff", projectId, sha, "patch"],
		queryFn: () => getCommitDiff(projectId, sha, "patch"),
	});
	if (isLoading) {
		return <p className="px-4 py-3 text-xs text-[var(--color-ink-3)]">loading…</p>;
	}
	if (isError) {
		return <p className="px-4 py-3 text-xs text-[var(--color-destructive)]">变更详情加载失败</p>;
	}
	const files = parseDocumentPatch(data?.patch ?? "");
	if (files.length === 0) {
		return <p className="px-4 py-3 text-xs text-[var(--color-ink-3)]">本次提交没有可显示的文本变更</p>;
	}
	return (
		<div className="space-y-4 border-t border-[var(--color-rule)] px-4 py-4">
		{files.map((entry) => (
			<div key={entry.path} className="overflow-hidden rounded-[var(--radius)] border border-[var(--color-rule)]">
				<button
					type="button"
					onClick={() => onOpen(entry.path)}
					className="flex w-full items-center gap-2 bg-[var(--color-paper-2)] px-3 py-2 text-left font-mono text-xs text-[var(--color-accent)] hover:underline"
				>
					<FileText className="size-3.5 shrink-0" />
					<span className="truncate">{entry.path}</span>
				</button>
				{entry.hunks.map((hunk, index) => (
					<div key={`${hunk.oldStart}-${hunk.newStart}-${index}`} className="border-t border-[var(--color-rule)]">
						<div className="flex flex-wrap items-center gap-x-3 gap-y-1 bg-[var(--color-surface-accent)] px-3 py-1.5 font-mono text-[11px]">
							<span className="text-[var(--color-accent)]">{entry.path}:L{hunk.newStart}</span>
							{hunk.heading && <span className="text-[var(--color-ink-2)]">§ {hunk.heading}</span>}
						</div>
						<pre className="overflow-x-auto py-1 text-[11px] leading-5">
							{hunk.lines.map((line, lineIndex) => {
								const number = line.kind === "delete" ? line.oldLine : line.newLine;
								const marker = line.kind === "add" ? "+" : line.kind === "delete" ? "−" : " ";
								return (
									<div
										key={`${lineIndex}-${line.kind}`}
										className={line.kind === "add" ? "bg-[oklch(94%_0.04_145)] text-[oklch(38%_0.11_145)] dark:bg-[oklch(25%_0.04_145)] dark:text-[oklch(78%_0.1_145)]" : line.kind === "delete" ? "bg-[oklch(94%_0.035_25)] text-[var(--color-destructive)] dark:bg-[oklch(25%_0.035_25)]" : "text-[var(--color-ink-3)]"}
									>
										<span className="inline-block w-12 select-none pr-2 text-right opacity-60">{number}</span>
										<span className="inline-block w-5 select-none text-center">{marker}</span>
										<code>{line.content || " "}</code>
									</div>
								);
							})}
						</pre>
					</div>
				))}
			</div>
		))}
		</div>
	);
}

function CommitChange({ projectId, commit, onOpen }: { projectId: string; commit: CommitSummary; onOpen: (path: string) => void }) {
	const [expanded, setExpanded] = useState(false);
	return (
		<article className="border-b border-[var(--color-rule)] last:border-b-0">
			<button type="button" onClick={() => setExpanded((value) => !value)} className="flex w-full items-start gap-3 px-4 py-3 text-left hover:bg-[var(--color-surface-accent)]" aria-expanded={expanded}>
				<ChevronDown className={`mt-0.5 size-4 shrink-0 text-[var(--color-ink-3)] transition-transform ${expanded ? "" : "-rotate-90"}`} />
				<div className="min-w-0 flex-1">
					<p className="text-sm font-medium text-[var(--color-ink)]">{commit.message || "（无提交信息）"}</p>
					<p className="mt-1 text-xs text-[var(--color-ink-3)]">{commit.author} · {formatDateTime(commit.date)}</p>
				</div>
				<code className="shrink-0 text-xs text-[var(--color-accent)]">{commit.sha.slice(0, 8)}</code>
			</button>
			{expanded && <ChangeDetails projectId={projectId} sha={commit.sha} onOpen={onOpen} />}
		</article>
	);
}

/** Project-level roadmap of recent document changes, intended for humans and agents. */
export function ProjectChanges({ projectId, onOpen }: { projectId: string; onOpen: (path: string) => void }) {
	const {
		data,
		isLoading,
		isError,
		hasNextPage,
		fetchNextPage,
		isFetchingNextPage,
	} = useInfiniteQuery({
		queryKey: ["commits", projectId],
		queryFn: ({ pageParam }) => listCommits(projectId, 20, pageParam),
		initialPageParam: 0,
		getNextPageParam: (lastPage, pages) =>
			lastPage.commits.length === 20
				? pages.reduce((total, page) => total + page.commits.length, 0)
				: undefined,
		enabled: projectId.length > 0,
	});
	const commits = data?.pages.flatMap((page) => page.commits) ?? [];
	return (
		<section className="mt-10 space-y-3" aria-label="项目变更记录">
			<div className="flex items-center justify-between">
				<div>
					<p className="mono-label flex items-center gap-2 text-[var(--color-ink-3)]"><GitCommitHorizontal className="size-3.5" />changes · roadmap</p>
					<p className="mt-1 text-xs text-[var(--color-ink-3)]">展开提交可查看文档路径、标题位置、行号和实际增删内容</p>
				</div>
				{data && <span className="mono-label text-[var(--color-ink-3)]">已加载 {commits.length} 次</span>}
			</div>
			<div className="hairline-panel overflow-hidden">
				{isLoading && <p className="px-4 py-6 text-center text-sm text-[var(--color-ink-3)]">loading…</p>}
				{isError && <p className="px-4 py-6 text-center text-sm text-[var(--color-destructive)]">变更记录加载失败</p>}
				{data && commits.length === 0 && <p className="px-4 py-6 text-center text-sm text-[var(--color-ink-2)]">还没有文档变更</p>}
				{commits.map((commit) => <CommitChange key={commit.sha} projectId={projectId} commit={commit} onOpen={onOpen} />)}
				{hasNextPage && (
					<button
						type="button"
						onClick={() => void fetchNextPage()}
						disabled={isFetchingNextPage}
						className="w-full border-t border-[var(--color-rule)] px-4 py-3 text-xs text-[var(--color-accent)] hover:bg-[var(--color-surface-accent)] disabled:opacity-60"
					>
						{isFetchingNextPage ? "加载中…" : "加载更早的改动"}
					</button>
				)}
			</div>
		</section>
	);
}
