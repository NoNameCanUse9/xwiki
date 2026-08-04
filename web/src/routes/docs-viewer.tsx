import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useBlocker, useNavigate, useParams } from "react-router-dom";
import {
	ArrowLeft,
	ChevronRight,
	CornerDownRight,
	FileText,
	Folder,
	History,
	Lock,
	Share2,
} from "lucide-react";
import { toast } from "sonner";
import { Button, buttonVariants } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { getRevision, submitChangeset } from "@/lib/api/changesets";
import { fileHistory } from "@/lib/api/history";
import { searchProject } from "@/lib/api/search";
import { createShare } from "@/lib/api/shares";
import {
	acquireLock,
	forceReleaseLock,
	heartbeatLock,
	lockFromError,
	releaseLock,
	type EditLock,
} from "@/lib/api/locks";
import { getPage, getTree, type TreeEntry } from "@/lib/api/docs";
import CommandPalette from "@/components/editor/command-palette";
import RichEditor from "@/components/editor/rich-editor";
import FileMenu from "@/components/editor/file-menu";
import ImportFilesButton from "@/components/editor/import-files";
import NewEntryButton from "@/components/editor/new-entry";
import RowActions from "@/components/editor/row-actions";
import AttachmentsPanel from "@/components/editor/attachments";
import { enhanceRenderedMarkdown } from "@/components/editor/markdown-render";
import {
	extractToc,
	TocPanel,
	VersionPanel,
	useVersionedPage,
	type TocEntry,
} from "@/components/editor/version-toc";
import { backlinks } from "@/lib/api/search";

function sanitizeHtml(html: string): string {
	return html
		.replace(/<script[\s\S]*?<\/script>/gi, "")
		.replace(/<iframe[\s\S]*?<\/iframe>/gi, "")
		.replace(/<object[\s\S]*?<\/object>/gi, "")
		.replace(/<embed[\s\S]*?<\/embed>/gi, "")
		.replace(/on\w+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)/gi, "")
		.replace(
			/(href|src|xlink:href)\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)/gi,
			(_m, attr, val) => {
				const v = val
					.replace(/^["']|["']$/g, "")
					.trim()
					.toLowerCase();
				return v.startsWith("javascript:") || v.startsWith("data:text/html")
					? `${attr}=""`
					: `${attr}=${val}`;
			},
		);
}

function Breadcrumbs({
	projectId,
	filePath,
}: {
	projectId: string;
	filePath: string;
}) {
	const segments = filePath.split("/").filter(Boolean);
	return (
		<nav
			aria-label="面包屑"
			className="mono-label flex flex-wrap items-center gap-1 text-[var(--color-ink-3)]"
		>
			<Link
				to={`/projects/${projectId}/docs`}
				className="hover:text-[var(--color-accent)]"
			>
				docs
			</Link>
			{segments.map((seg, i) => {
				const prefix = segments.slice(0, i + 1).join("/");
				const isFile = i === segments.length - 1;
				return (
					<span key={prefix} className="flex items-center gap-1">
						<ChevronRight className="size-3" />
						{isFile ? (
							<span className="text-[var(--color-ink)]">{seg}</span>
						) : (
							<Link
								to={`/projects/${projectId}/docs/${prefix}`}
								className="hover:text-[var(--color-accent)]"
							>
								{seg}
							</Link>
						)}
					</span>
				);
			})}
		</nav>
	);
}

function MarkdownArticle({
	html,
	onNavigate,
	onToc,
}: {
	html: string;
	onNavigate: (path: string) => void;
	onToc: (entries: TocEntry[]) => void;
}) {
	const ref = useRef<HTMLElement>(null);
	useEffect(() => {
		if (ref.current) {
			void enhanceRenderedMarkdown(ref.current).then(() => {
				onToc(extractToc(ref.current!));
			});
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [html]);
	return (
		<article
			ref={ref}
			className="prose-agentdocs"
			dangerouslySetInnerHTML={{ __html: html }}
			onClick={(e) => {
				const anchor = (e.target as HTMLElement).closest("a");
				if (!anchor) return;
				const href = anchor.getAttribute("href") ?? "";
				if (href.startsWith("/projects/")) {
					e.preventDefault();
					onNavigate(href);
				}
			}}
		/>
	);
}

function BacklinksPanel({
	projectId,
	filePath,
}: {
	projectId: string;
	filePath: string;
}) {
	const navigate = useNavigate();
	const { data } = useQuery({
		queryKey: ["backlinks", projectId, filePath],
		queryFn: () => backlinks(projectId, filePath),
		enabled: filePath.length > 0,
	});
	const items = data?.backlinks ?? [];
	return (
		<section className="space-y-3">
			<p className="mono-label flex items-center gap-2 text-[var(--color-ink-3)]">
				<CornerDownRight className="size-3.5" />
				backlinks · {items.length}
			</p>
			{items.length === 0 ? (
				<p className="hairline-panel px-4 py-5 text-center text-sm text-[var(--color-ink-2)]">
					暂无其他页面引用本文档
				</p>
			) : (
				<div className="hairline-panel divide-y divide-[var(--color-rule)] px-4">
					{items.map((b) => (
						<button
							key={b.source}
							type="button"
							onClick={() =>
								navigate(`/projects/${projectId}/docs/${b.source}`)
							}
							className="block w-full py-2.5 text-left hover:bg-[var(--color-surface-accent)]"
						>
							<span className="font-mono text-xs text-[var(--color-accent)]">
								{b.source}
							</span>
							<span className="ml-3 text-sm text-[var(--color-ink-2)]">
								{b.snippet}
							</span>
						</button>
					))}
				</div>
			)}
		</section>
	);
}

function FileExplorer({
	projectId,
	dirPath,
	depth,
	defaultExpanded,
	onOpen,
}: {
	projectId: string;
	dirPath: string;
	depth: number;
	defaultExpanded?: boolean;
	onOpen: (path: string) => void;
}) {
	const [expanded] = useState(defaultExpanded ?? false);
	const { data, isLoading } = useQuery({
		queryKey: ["dir", projectId, dirPath],
		queryFn: () => getTree(projectId, dirPath),
		enabled: expanded || depth === 0,
	});
	const dirs = (data?.tree ?? []).filter((e) => e.type === "tree");
	const files = (data?.tree ?? []).filter(
		(e) => e.type === "blob" && e.path !== "_sidebar.md",
	);
	const isRoot = depth === 0;
	const itemCount = dirs.length + files.length;

	if (isRoot) {
		return (
			<section>
				<div className="mb-1 flex items-center gap-2 px-4 py-2.5">
					<span className="mono-label text-[var(--color-ink-3)]">
						root · {itemCount} {itemCount === 1 ? "item" : "items"}
					</span>
				</div>
				{isLoading && (
					<p className="px-4 py-3 text-xs text-[var(--color-ink-3)]">
						loading…
					</p>
				)}
				{!isLoading && itemCount === 0 && (
					<p className="hairline-panel mx-4 my-6 px-4 py-8 text-center text-sm text-[var(--color-ink-2)]">
						空目录
					</p>
				)}
				{!isLoading && itemCount > 0 && (
					<div className="divide-y divide-[var(--color-rule)] border-t border-[var(--color-rule)]">
						{dirs.map((d) => (
							<ExpandableRow
								key={d.path}
								projectId={projectId}
								entry={d}
								depth={0}
								onOpen={onOpen}
							/>
						))}
						{files.map((f) => (
							<div
								key={f.path}
								className="group flex w-full items-center gap-3 px-4 py-2.5 text-sm hover:bg-[var(--color-surface-accent)]"
							>
								<button
									type="button"
									onClick={() => onOpen(f.path)}
									aria-label={f.name}
									className="flex min-w-0 flex-1 items-center gap-3 text-left"
								>
									<FileText className="size-4 shrink-0 text-[var(--color-ink-3)]" />
									<span className="truncate text-[var(--color-ink)]">
										{f.name}
									</span>
								</button>
								<span className="shrink-0 opacity-0 transition-opacity group-hover:opacity-100">
									<RowActions
										projectId={projectId}
										path={f.path}
										type="file"
										onDone={() => {}}
										onOpen={() => onOpen(f.path)}
									/>
								</span>
							</div>
						))}
					</div>
				)}
			</section>
		);
	}
	return null;
}

function ExpandableRow({
	projectId,
	entry,
	depth,
	onOpen,
}: {
	projectId: string;
	entry: TreeEntry;
	depth: number;
	onOpen: (path: string) => void;
}) {
	const [expanded, setExpanded] = useState(false);
	const { data } = useQuery({
		queryKey: ["dir", projectId, entry.path],
		queryFn: () => getTree(projectId, entry.path),
		enabled: expanded,
	});
	const children = data?.tree ?? [];
	const childDirs = children.filter((e) => e.type === "tree");
	const childFiles = children.filter(
		(e) => e.type === "blob" && e.path !== "_sidebar.md",
	);
	const childCount = childDirs.length + childFiles.length;
	const indent = depth * 20 + 28;
	return (
		<div>
			<div className="group flex w-full items-center gap-3 px-4 py-2.5 text-sm hover:bg-[var(--color-surface-accent)]">
				<button
					type="button"
					onClick={() => setExpanded((v) => !v)}
					aria-label={entry.name}
					className="flex min-w-0 flex-1 items-center gap-3 text-left"
				>
					<ChevronRight
						className={`size-4 shrink-0 text-[var(--color-ink-3)] transition-transform ${
							expanded ? "rotate-90" : ""
						}`}
					/>
					<Folder className="size-4 shrink-0 text-[var(--color-accent)]" />
					<span className="truncate text-[var(--color-ink)]">{entry.name}</span>
					{!expanded && childCount > 0 && (
						<span className="mono-label ml-auto text-[var(--color-ink-3)]">
							{childCount}
						</span>
					)}
				</button>
				<span className="shrink-0 opacity-0 transition-opacity group-hover:opacity-100">
					<RowActions
						projectId={projectId}
						path={entry.path}
						type="folder"
						onDone={() => setExpanded(false)}
					/>
				</span>
			</div>
			{expanded && (
				<div>
					{childDirs.map((d) => (
						<div key={d.path} style={{ paddingLeft: `${indent}px` }}>
							<ExpandableRow
								projectId={projectId}
								entry={d}
								depth={depth + 1}
								onOpen={onOpen}
							/>
						</div>
					))}
					{childFiles.map((f) => (
						<div
							key={f.path}
							className="group flex w-full items-center gap-3 text-sm hover:bg-[var(--color-surface-accent)]"
							style={{ paddingLeft: `${indent}px` }}
						>
							<button
								type="button"
								onClick={() => onOpen(f.path)}
								aria-label={f.name}
								className="flex min-w-0 flex-1 items-center gap-3 px-4 py-2.5 text-left"
							>
								<span className="size-4 shrink-0" />
								<FileText className="size-4 shrink-0 text-[var(--color-ink-3)]" />
								<span className="truncate text-[var(--color-ink)]">
									{f.name}
								</span>
							</button>
							<span className="shrink-0 px-4 opacity-0 transition-opacity group-hover:opacity-100">
								<RowActions
									projectId={projectId}
									path={f.path}
									type="file"
									onDone={() => {}}
									onOpen={() => onOpen(f.path)}
								/>
							</span>
						</div>
					))}
				</div>
			)}
		</div>
	);
}

export default function DocsViewerPage() {
	const { id = "", "*": filePath = "" } = useParams();
	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const [editing, setEditing] = useState(false);
	const [dirty, setDirty] = useState(false);
	// The live draft lives in a ref so typing never re-renders the page;
	// `seed` is the initial content the editor mounts with (a restored
	// local draft, or the server raw content).
	const draftRef = useRef("");
	const dirtyRef = useRef(false);
	const [seed, setSeed] = useState<string | null>(null);
	const [saving, setSaving] = useState(false);
	// Lock state machine: idle (locked/read-only) -> opening (acquiring) ->
	// held (editing) | blocked (another user holds the page).
	const [lockState, setLockState] = useState<
		"idle" | "opening" | "held" | "blocked"
	>("idle");
	const [lockHolder, setLockHolder] = useState<EditLock | null>(null);
	const [saveState, setSaveState] = useState<"idle" | "saving" | "saved">("idle");
	const [commitMessage, setCommitMessage] = useState("");
	const [commitOpen, setCommitOpen] = useState(false);
	const [restoring, setRestoring] = useState(false);

	// Restore the current page to a historical version (from atSha).
	const restoreVersion = async () => {
		if (!atSha) return;
		setRestoring(true);
		try {
			const raw = await getPage(id, filePath, "raw", atSha);
			const rev = await getRevision(id);
			await submitChangeset(id, {
				base_revision: rev.revision,
				message: `恢复到版本 ${atSha.slice(0, 7)}`,
				changes: [{ op: "update", path: filePath, content: raw.content }],
			});
			setAtSha(null);
			toast.success("已恢复到此版本");
			await queryClient.invalidateQueries({ queryKey: ["docs"] });
			await queryClient.invalidateQueries({ queryKey: ["tree"] });
			await queryClient.invalidateQueries({ queryKey: ["docs", "raw"] });
		} catch (err) {
			toast.error(err instanceof Error ? err.message : "恢复失败");
		} finally {
			setRestoring(false);
		}
	};
	const [showHistory, setShowHistory] = useState(false);
	const [showAttachments, setShowAttachments] = useState(false);
	const [showBacklinks, setShowBacklinks] = useState(false);
	const [atSha, setAtSha] = useState<string | null>(null);
	const [tocEntries, setTocEntries] = useState<TocEntry[]>([]);
	const [searchQuery, setSearchQuery] = useState("");
	const [searchResults, setSearchResults] = useState<Array<{
		path: string;
		snippet: string;
	}> | null>(null);
	const [searching, setSearching] = useState(false);

	const showHome = !filePath;
	const isDirPath = filePath.length > 0 && filePath.endsWith("/");
	const dirPath = isDirPath ? filePath.slice(0, -1) : "";

	// Custom sidebar menu from _sidebar.md at the repo root (OtterWiki-style).
	const sidebarQuery = useQuery({
		queryKey: ["docs", "sidebar", id],
		queryFn: () => getPage(id, "_sidebar.md"),
		enabled: true,
	});
	const sidebarItems = useMemo(() => {
		const raw = sidebarQuery.data?.content ?? "";
		const items: Array<{ label: string; path: string }> = [];
		const re = /^[-*]\s+\[([^\]]+)\]\(([^)]+)\)/gm;
		let m: RegExpExecArray | null;
		while ((m = re.exec(raw)) !== null) {
			items.push({ label: m[1], path: m[2] });
		}
		return items;
	}, [sidebarQuery.data]);

	const pageQuery = useVersionedPage(id, filePath, atSha);

	const historyQuery = useQuery({
		queryKey: ["history", id, filePath],
		queryFn: () => fileHistory(id, filePath),
		enabled: showHistory && !showHome,
	});

	const runSearch = async () => {
		const q = searchQuery.trim();
		if (!q) return;
		setSearching(true);
		try {
			const res = await searchProject(id, q);
			setSearchResults(res.results);
		} catch {
			setSearchResults([]);
		} finally {
			setSearching(false);
		}
	};

	const selectVersion = (sha: string | null) => {
		setAtSha(sha);
		setTocEntries([]);
	};

	// Share the current page: creates/reuses a /share/{token} link and copies
	// it to the clipboard so it can be handed out directly.
	const sharePage = async () => {
		if (!filePath) return;
		try {
			const { url } = await createShare(id, filePath);
			const full = `${window.location.origin}${url}`;
			try {
				await navigator.clipboard.writeText(full);
				toast.success(`分享链接已复制：${full}`);
			} catch {
				window.prompt("分享链接（复制）", full);
			}
		} catch (err) {
			toast.error(err instanceof Error ? err.message : "生成分享链接失败");
		}
	};

	// Edit flow: load raw content, submit an update changeset on save.
	const rawQuery = useQuery({
		queryKey: ["docs", "raw", id, filePath],
		queryFn: () => getPage(id, filePath, "raw"),
		enabled: editing && !showHome,
	});

	// --- Exclusive edit lock: unlock to edit, lock to commit & finish. ---

	// Keyed per keystroke: updates a ref only (no re-render). The first edit
	// flips dirty once; further typing stays cheap.
	const handleEditorChange = (md: string) => {
		draftRef.current = md;
		if (!dirtyRef.current) {
			dirtyRef.current = true;
			setDirty(true);
		}
	};

	const markClean = () => {
		dirtyRef.current = false;
		setDirty(false);
	};

	const draftKey = (projectId: string, path: string) =>
		`agentdocs:draft:${projectId}:${path}`;

	const startEditing = async () => {
		if (lockState === "opening") return;
		setLockState("opening");
		const saved = localStorage.getItem(draftKey(id, filePath));
		try {
			await acquireLock(id, filePath);
			setSeed(saved ?? null);
			markClean();
			setLockHolder(null);
			setLockState("held");
			setEditing(true);
		} catch (err) {
			const holder = lockFromError(err);
			if (holder) {
				setLockHolder(holder);
				setLockState("blocked");
				toast.error(`${holder.username} 正在编辑此页面`);
			} else {
				toast.error(err instanceof Error ? err.message : "无法获取编辑锁");
				setLockState("idle");
			}
		}
	};

	// Commit the current draft as one changeset (Cmd/Ctrl+S). One edit = one
	// commit; message stays empty so the backend stamps 时间 + 用户名.
	const commitDraft = async () => {
		if (!editing || saving) return;
		setSaving(true);
		setSaveState("saving");
		try {
			const rev = await getRevision(id);
			await submitChangeset(id, {
				base_revision: rev.revision,
				// 留空则后端生成默认：时间 + 操作者 修改 <path>
				message: commitMessage.trim(),
				changes: [{ op: "update", path: filePath, content: draftRef.current }],
			});
			setSaveState("saved");
			toast.success("已提交");
			markClean();
			setCommitMessage("");
			localStorage.removeItem(draftKey(id, filePath));
			await queryClient.invalidateQueries({ queryKey: ["docs"] });
			await queryClient.invalidateQueries({ queryKey: ["tree"] });
			// Keep the raw cache in sync so a re-edit of the same file shows
			// the just-committed content, not the pre-commit snapshot.
			await queryClient.invalidateQueries({ queryKey: ["docs", "raw"] });
		} catch (err) {
			setSaveState("idle");
			if ((err as { status?: number })?.status === 409) {
				toast.error("文档已被他人修改，请刷新后重试");
				setEditing(false);
				setLockState("idle");
			} else {
				toast.error(err instanceof Error ? err.message : "保存失败");
			}
		} finally {
			setSaving(false);
		}
	};

	// Lock the page again: commit any pending edits, then release the lock.
	const lockAndCommit = async () => {
		if (!editing || saving) return;
		setCommitOpen(false);
		try {
			if (dirty) await commitDraft();
		} catch {
			return; // commit failed; stay editing so nothing is lost
		}
		await releaseLock(id, filePath).catch(() => {});
		setEditing(false);
		setLockState("idle");
		markClean();
		localStorage.removeItem(draftKey(id, filePath));
	};

	// Any signed-in user may force a held lock open (holder's draft is lost).
	const forceUnlock = async () => {
		if (!lockHolder) return;
		if (
			!window.confirm(
				`强制解锁将中断 ${lockHolder.username} 的编辑并丢弃其未提交修改，确定？`,
			)
		)
			return;
		try {
			await forceReleaseLock(id, filePath);
			setLockHolder(null);
			setLockState("idle");
			toast.success("已强制解锁，可重新获取编辑锁");
		} catch (err) {
			toast.error(err instanceof Error ? err.message : "强制解锁失败");
		}
	};

	// Renew the lease every 30s while editing; a lost lock (expired or
	// force-released) drops the editor back to read-only.
	useEffect(() => {
		if (!editing || !filePath) return;
		const beat = () => {
			heartbeatLock(id, filePath).catch(() => {
				setEditing(false);
				setLockState("idle");
				markClean();
				localStorage.removeItem(draftKey(id, filePath));
				toast.error("编辑锁已失效，已回到只读模式");
			});
		};
		beat();
		const t = window.setInterval(beat, 30_000);
		return () => window.clearInterval(t);
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [editing, id, filePath]);

	// Debounced local draft persistence (never committed; survives refresh).
	useEffect(() => {
		if (!editing || !dirtyRef.current) return;
		const t = window.setTimeout(() => {
			try {
				localStorage.setItem(draftKey(id, filePath), draftRef.current);
			} catch {
				// quota exceeded — ignore, the draft lives in memory anyway
			}
		}, 800);
		return () => window.clearTimeout(t);
	}, [editing, dirty, id, filePath]);

	// Navigating to a different target (another file, a directory, or the docs
	// root) releases the lock and ends the edit session. Without this,
	// `editing` survives SPA navigation and the editor UI leaks onto the
	// file-list views. (`editing` is intentionally not a dependency: starting
	// to edit must not cancel itself.)
	useEffect(() => {
		if (!editing) return;
		void releaseLock(id, filePath).catch(() => {});
		setEditing(false);
		setLockState("idle");
		markClean();
		localStorage.removeItem(draftKey(id, filePath));
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [id, filePath]);

	// Cmd/Ctrl+S commits; beforeunload guards against losing unsaved edits
	// and best-effort releases the lock so the page is not left locked.
	useEffect(() => {
		const onKey = (e: KeyboardEvent) => {
			if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") {
				e.preventDefault();
				if (editing && !saving) void commitDraft();
			}
		};
		window.addEventListener("keydown", onKey);
		return () => window.removeEventListener("keydown", onKey);
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [editing, saving, id, filePath]);

	useEffect(() => {
		if (!dirty || !editing) return;
		const onBeforeUnload = (e: BeforeUnloadEvent) => {
			e.preventDefault();
			e.returnValue = "";
			void releaseLock(id, filePath).catch(() => {});
		};
		window.addEventListener("beforeunload", onBeforeUnload);
		return () => window.removeEventListener("beforeunload", onBeforeUnload);
	}, [dirty, editing, id, filePath]);

	// Block SPA navigation while there are unsaved edits (beforeunload only
	// covers full-page unload). The blocker effect shows a confirm dialog;
	// 放弃 keeps editing, proceeding discards the draft.
	const blocker = useBlocker(editing && dirty);
	useEffect(() => {
		if (blocker.state !== "blocked") return;
		if (window.confirm("有未保存的修改，确定放弃？")) {
			markClean();
			blocker.proceed();
		} else {
			blocker.reset();
		}
	}, [blocker]);

	const content = pageQuery.data;
	const loading = pageQuery.isLoading;
	const error = pageQuery.isError;

	return (
		<div className="flex min-h-screen">
			{!showHome && (
				<aside className="fixed inset-y-0 left-0 z-30 hidden w-64 flex-col border-r border-[var(--color-rule)] bg-[var(--color-paper-2)] sm:flex">
					<div className="border-b border-[var(--color-rule)] px-4 py-3">
						<Link
							to="/"
							className="mono-label flex items-center gap-2 text-[var(--color-ink-3)] hover:text-[var(--color-accent)]"
						>
							<ArrowLeft className="size-3.5" />
							workspace
						</Link>
					</div>
					{!showHome && (
						<div className="scrollbar-hidden flex min-h-0 flex-1 flex-col border-b border-[var(--color-rule)] overflow-x-hidden">
							<div className="scrollbar-hidden min-h-0 flex-1 overflow-y-auto overflow-x-hidden p-2">
								<TocPanel entries={tocEntries} />
							</div>
							<div className="scrollbar-hidden min-h-0 flex-1 overflow-y-auto border-t border-[var(--color-rule)] p-2">
								<VersionPanel
									projectId={id}
									filePath={filePath}
									currentVersion={atSha}
									onSelect={selectVersion}
								/>
							</div>
						</div>
					)}
					<div className="scrollbar-hidden min-h-0 overflow-y-auto overflow-x-hidden p-2">
						{sidebarItems.length > 0 && (
							<nav className="space-y-0.5">
								<p className="mono-label px-2 pb-1 text-[var(--color-ink-3)]">
									menu
								</p>
								{sidebarItems.map((item) => (
									<button
										key={item.path}
										type="button"
										onClick={() =>
											navigate(`/projects/${id}/docs/${item.path}`)
										}
										className="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm text-[var(--color-ink-2)] hover:bg-[var(--color-surface-accent)] hover:text-[var(--color-ink)]"
									>
										<span className="truncate">{item.label}</span>
									</button>
								))}
							</nav>
						)}
					</div>
				</aside>
			)}

			<CommandPalette projectId={id} />
			<Dialog open={commitOpen} onOpenChange={setCommitOpen}>
				<DialogContent>
					<DialogHeader>
						<DialogTitle>提交修改</DialogTitle>
						<DialogDescription>
							填写 commit message，留空则使用默认（时间 + 用户名）。提交后文档将被锁定（只读）。
						</DialogDescription>
					</DialogHeader>
					<div className="space-y-2">
						<Input
							aria-label="commit message"
							placeholder="commit message（留空默认：时间 + 用户名）"
							value={commitMessage}
							onChange={(e) => setCommitMessage(e.target.value)}
							onKeyDown={(e) => {
								if (e.key === "Enter") void lockAndCommit();
							}}
							className="font-mono text-xs"
							autoFocus
						/>
					</div>
					<div className="flex justify-end gap-2">
						<Button variant="outline" onClick={() => setCommitOpen(false)}>
							取消
						</Button>
						<Button
							onClick={() => void lockAndCommit()}
							disabled={saving}
						>
							<Lock className="size-3.5" />
							{saving ? "提交中…" : "提交并上锁"}
						</Button>
					</div>
				</DialogContent>
			</Dialog>
			<div className="flex w-full flex-col">
				<header className="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--color-rule)] px-6 py-3">
					<div className="flex items-center gap-3">
						{showHome && (
							<Link
								to="/"
								className={buttonVariants({ variant: "outline", size: "sm" })}
							>
								<ArrowLeft className="size-4" />
								返回工作台
							</Link>
						)}
						<Breadcrumbs projectId={id} filePath={filePath} />
					</div>
					<form
						className="flex items-center gap-2"
						onSubmit={(e) => {
							e.preventDefault();
							void runSearch();
						}}
					>
						{!showHome && !isDirPath && !atSha && (
							<div className="mr-1 flex items-center gap-1.5">
								{editing ? (
									<>
										<span className="mono-label text-[var(--color-ink-3)]">
											{saveState === "saving"
												? "提交中…"
												: saveState === "saved"
													? "已提交"
													: dirty
														? "自动保存草稿中"
														: "已是最新"}
										</span>
										<Button
											size="sm"
											onClick={() => setCommitOpen(true)}
											disabled={saving}
										>
											<Lock className="size-3.5" />
											上锁并提交
										</Button>
									</>
								) : (
									<>
										<Button
											variant="outline"
											size="sm"
											className="gap-2"
											onClick={() => void startEditing()}
											disabled={lockState === "opening"}
										>
											<Lock className="size-3.5" />
											{lockState === "opening" ? "获取中…" : "解锁编辑"}
										</Button>
										<Button
											variant="ghost"
											size="sm"
											className="gap-2 text-[var(--color-ink-3)]"
											onClick={() => setShowHistory((v) => !v)}
										>
											<History className="size-3.5" />
											历史
										</Button>
										<Button
											variant="ghost"
											size="sm"
											className="gap-2 text-[var(--color-ink-3)]"
											onClick={() => void sharePage()}
										>
											<Share2 className="size-3.5" />
											分享
										</Button>
										<FileMenu
											projectId={id}
											filePath={filePath}
											items={{
												onEdit: () => void startEditing(),
												onToggleHistory: () => setShowHistory((v) => !v),
												onToggleAttachments: () => setShowAttachments((v) => !v),
												onToggleBacklinks: () => setShowBacklinks((v) => !v),
											}}
										/>
									</>
								)}
							</div>
						)}
						{!showHome && <ImportFilesButton projectId={id} />}
						<input
							aria-label="搜索文档"
							value={searchQuery}
							onChange={(e) => {
								setSearchQuery(e.target.value);
								setSearchResults(null);
							}}
							placeholder="搜索…"
							className="h-8 w-48 rounded-[var(--radius)] border border-[var(--color-rule)] bg-[var(--color-paper)] px-3 font-mono text-xs text-[var(--color-ink)] placeholder:text-[var(--color-ink-3)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]"
						/>
						<Button
							type="submit"
							variant="outline"
							size="sm"
							disabled={searching}
						>
							{searching ? "…" : "搜索"}
						</Button>
					</form>
				</header>
				{searchResults && (
					<div className="border-b border-[var(--color-rule)] bg-[var(--color-paper-2)] px-6 py-3">
						{searchResults.length === 0 ? (
							<p className="mono-label text-[var(--color-ink-3)]">no results</p>
						) : (
							<div className="space-y-1">
								<p className="mono-label text-[var(--color-ink-3)]">
									{searchResults.length} results
								</p>
								{searchResults.map((r) => (
									<button
										key={r.path}
										type="button"
										onClick={() => {
											navigate(`/projects/${id}/docs/${r.path}`);
											setSearchResults(null);
											setSearchQuery("");
										}}
										className="block w-full rounded-sm px-2 py-1.5 text-left hover:bg-[var(--color-surface-accent)]"
									>
										<span className="font-mono text-xs text-[var(--color-accent)]">
											{r.path}
										</span>
										<span className="ml-3 text-sm text-[var(--color-ink-2)]">
											{r.snippet}
										</span>
									</button>
								))}
							</div>
						)}
					</div>
				)}

				<main className="flex-1 px-6 py-8 sm:ml-64 sm:px-10">
					<div
						className={`mx-auto w-full transition-[max-width] duration-200 ${
							editing ? "max-w-4xl" : "max-w-3xl"
						}`}
					>
						{loading && (
							<p className="mono-label text-[var(--color-ink-3)]">loading…</p>
						)}
						{error && (
							<div className="hairline-panel px-6 py-10 text-center">
								<p className="font-display text-lg font-semibold text-[var(--color-ink)]">
									文档不存在
								</p>
								<p className="mt-2 text-sm text-[var(--color-ink-2)]">
									请从左侧文档树选择其他页面。
								</p>
							</div>
						)}
						{lockState === "blocked" && lockHolder && (
							<div className="mb-4 flex items-center justify-between gap-3 rounded-[var(--radius)] border border-[var(--color-accent)] bg-[var(--color-surface-accent)] px-4 py-2.5">
								<p className="mono-label text-[var(--color-ink-2)]">
									{lockHolder.username} 正在编辑此页面，当前只读
								</p>
								<Button
									size="sm"
									variant="outline"
									onClick={() => void forceUnlock()}
								>
									强制解锁
								</Button>
							</div>
						)}
						{atSha && (
							<div className="mb-4 flex items-center justify-between gap-3 rounded-[var(--radius)] border border-[var(--color-accent)] bg-[var(--color-surface-accent)] px-4 py-2.5">
								<p className="mono-label text-[var(--color-ink-2)]">
									viewing historical version {atSha.slice(0, 7)}
								</p>
								<div className="flex items-center gap-2">
									<Button
										size="sm"
										variant="outline"
										onClick={() => void restoreVersion()}
										disabled={restoring}
									>
										{restoring ? "恢复中…" : "恢复到此版本"}
									</Button>
									<Button
										size="sm"
										variant="outline"
										onClick={() => selectVersion(null)}
									>
										返回最新版本
									</Button>
								</div>
							</div>
						)}
						{content && content.format === "html" && !editing && (
							<MarkdownArticle
								html={sanitizeHtml(content.content)}
								onToc={setTocEntries}
								onNavigate={(href) => {
									const m = href.match(/^\/projects\/[^/]+\/docs\/(.+)$/);
									if (m) navigate(`/projects/${id}/docs/${m[1]}`);
								}}
							/>
						)}
						{content && content.format === "raw" && !editing && (
							<pre className="code-card overflow-x-auto p-4">
								{content.content}
							</pre>
						)}
						{isDirPath && (
							<FileExplorer
								projectId={id}
								dirPath={dirPath}
								depth={0}
								defaultExpanded
								onOpen={(p) => navigate(`/projects/${id}/docs/${p}`)}
							/>
						)}
						{showHome && !loading && !error && (
							<div className="mb-4 flex items-center justify-between gap-3">
								<span className="mono-label text-[var(--color-ink-3)]">
									root
								</span>
								<div className="flex items-center gap-2">
									<NewEntryButton projectId={id} />
									<ImportFilesButton projectId={id} />
								</div>
							</div>
						)}
						{showHome && !loading && !error && (
							<FileExplorer
								projectId={id}
								dirPath=""
								depth={0}
								defaultExpanded
								onOpen={(path) => navigate(`/projects/${id}/docs/${path}`)}
							/>
						)}
						{!showHome && !editing && showBacklinks && (
							<div className="mt-10">
								<BacklinksPanel projectId={id} filePath={filePath} />
							</div>
						)}
						{!showHome && !editing && showAttachments && (
							<div className="mt-10">
								<AttachmentsPanel projectId={id} />
							</div>
						)}
						{showHistory && !editing && (
							<div className="hairline-panel mt-4 px-5">
								<p className="mono-label py-3 text-[var(--color-ink-3)]">
									history · {filePath}
								</p>
								{historyQuery.isLoading && (
									<p className="mono-label pb-3 text-[var(--color-ink-3)]">
										loading…
									</p>
								)}
								{historyQuery.data?.commits.map((c) => (
									<div
										key={c.sha}
										className="flex items-center justify-between gap-3 border-t border-[var(--color-rule)] py-2.5"
									>
										<p className="truncate font-mono text-xs text-[var(--color-accent)]">
											{c.sha.slice(0, 8)}
										</p>
										<p className="min-w-0 flex-1 truncate text-sm text-[var(--color-ink)]">
											{c.message}
										</p>
										<p className="mono-label shrink-0 text-[var(--color-ink-3)]">
											{new Date(c.date).toLocaleDateString("zh-CN")}
										</p>
									</div>
								))}
							</div>
						)}
						{editing && !showHome && !isDirPath && (
							rawQuery.isLoading || rawQuery.isFetching ? (
								<p className="mono-label text-[var(--color-ink-3)]">
									loading…
								</p>
							) : (
								<>
									<RichEditor
										initialMarkdown={seed ?? rawQuery.data?.content ?? ""}
										onChange={handleEditorChange}
										onNavigateLink={(href) => {
											const m = href.match(
												/^\/projects\/[^/]+\/docs\/(.+)$/,
											);
											if (m) navigate(`/projects/${id}/docs/${m[1]}`);
										}}
									/>
								</>
							)
						)}
					</div>
				</main>
			</div>
		</div>
	);
}
