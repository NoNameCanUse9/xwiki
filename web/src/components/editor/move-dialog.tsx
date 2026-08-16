import { useCallback, useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ChevronRight, FileText, Folder } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ApiError } from "@/lib/api/client";
import {
	getRevision,
	submitChangeset,
	type ChangeInput,
} from "@/lib/api/changesets";
import { getTree, type TreeEntry } from "@/lib/api/docs";

/** Map a move failure to a user-facing Chinese message. */
function moveErrorMessage(err: unknown, target: string): string {
	if (err instanceof ApiError) {
		switch (err.code) {
			case "path_exists":
				return `目标路径 ${target} 已存在，请换一个名称或目标文件夹`;
			case "source_missing":
				return "源路径不存在，可能已被删除，请刷新后再试";
			case "revision_conflict":
				return "文档已被他人修改，请刷新后重试";
			default:
				return err.message;
		}
	}
	return "移动失败";
}

function isWithin(source: string, path: string): boolean {
	return path === source || path.startsWith(`${source}/`);
}

interface MoveDialogProps {
	projectId: string;
	source: string;
	isDir: boolean;
	open: boolean;
	onOpenChange: (open: boolean) => void;
	onDone: () => void;
}

export default function MoveDialog({
	projectId,
	source,
	isDir,
	open,
	onOpenChange,
	onDone,
}: MoveDialogProps) {
	const queryClient = useQueryClient();
	const defaultName = source.split("/").pop() ?? source;
	const sourceParent = source.includes("/")
		? source.slice(0, source.lastIndexOf("/"))
		: "";

	const [entries, setEntries] = useState<Map<string, TreeEntry[]>>(new Map());
	const [expanded, setExpanded] = useState<Set<string>>(new Set());
	const [selectedDir, setSelectedDir] = useState("");
	const [name, setName] = useState(defaultName);
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);

	const loadDir = useCallback(
		async (dir: string) => {
			try {
				const res = await getTree(projectId, dir);
				setEntries((prev) => new Map(prev).set(dir, res.tree));
				setError(null);
			} catch (err) {
				setError(
					err instanceof Error ? `加载 ${dir || "根目录"} 失败：${err.message}` : "加载目录失败",
				);
			}
		},
		[projectId],
	);

	// Open: reset state, then load root and expand ancestors down to the
	// source parent so the default selection is visible.
	useEffect(() => {
		if (!open) return;
		setEntries(new Map());
		setExpanded(new Set());
		setSelectedDir(sourceParent);
		setName(defaultName);
		setError(null);
		void (async () => {
			const chain: string[] = [""];
			let cur = sourceParent;
			while (cur) {
				chain.push(cur);
				const i = cur.lastIndexOf("/");
				if (i < 0) break;
				cur = cur.slice(0, i);
			}
			chain.reverse();
			for (const dir of chain) {
				await loadDir(dir);
				setExpanded((prev) => new Set(prev).add(dir));
			}
		})();
	}, [open, source, sourceParent, defaultName, loadDir]);

	const toggle = async (dir: string) => {
		if (expanded.has(dir)) {
			const next = new Set(expanded);
			next.delete(dir);
			setExpanded(next);
			return;
		}
		setExpanded((prev) => new Set(prev).add(dir));
		if (!entries.has(dir)) {
			await loadDir(dir);
		}
	};

	const target = selectedDir ? `${selectedDir}/${name.trim()}` : name.trim();

	const submit = async () => {
		const n = name.trim();
		if (!n || n.includes("/")) {
			toast.error("名称不能为空且不能包含斜杠");
			return;
		}
		if (target === source) {
			toast.error("目标与当前位置相同，无需移动");
			return;
		}
		setBusy(true);
		try {
			const rev = await getRevision(projectId);
			const changes: ChangeInput[] = [{ op: "move", path: source, new_path: target }];
			// Preflight: the dry run runs the same conflict checks as the real
			// submission (409 path_exists) without producing a commit.
			await submitChangeset(
				projectId,
				{ base_revision: rev.revision, message: "", changes },
				true,
			);
			await submitChangeset(projectId, {
				base_revision: rev.revision,
				message: "",
				changes,
			});
			toast.success(`已移动 → ${target}`);
			onOpenChange(false);
			await queryClient.invalidateQueries({ queryKey: ["tree"] });
			await queryClient.invalidateQueries({ queryKey: ["dir"] });
			onDone();
		} catch (err) {
			toast.error(moveErrorMessage(err, target));
		} finally {
			setBusy(false);
		}
	};

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="max-w-md">
				<DialogHeader>
					<DialogTitle>
						{isDir ? `移动目录 ${defaultName}` : `移动文档 ${defaultName}`}
					</DialogTitle>
					<DialogDescription>
						选择目标文件夹，可同时修改名称
					</DialogDescription>
				</DialogHeader>

				<div className="h-56 overflow-auto rounded-[var(--radius)] border border-[var(--color-rule)] bg-[var(--color-paper-2)]">
					{error ? (
						<p className="px-3 py-2 text-xs text-[var(--color-destructive)]">
							{error}
						</p>
					) : (
						<div className="py-1">
							{(entries.get("") ?? []).map((entry) => (
								<MoveRow
									key={entry.path}
									entry={entry}
									depth={0}
									source={source}
									isDirMove={isDir}
									entries={entries}
									expanded={expanded}
									selectedDir={selectedDir}
									onToggle={toggle}
									onSelect={setSelectedDir}
								/>
							))}
							{entries.size === 0 && (
								<p className="px-3 py-2 text-xs text-[var(--color-ink-3)]">
									加载中…
								</p>
							)}
						</div>
					)}
				</div>

				<div className="grid gap-1.5">
					<label className="text-xs text-[var(--color-ink-3)]" htmlFor="move-name">
						新名称
					</label>
					<Input
						id="move-name"
						value={name}
						onChange={(e) => setName(e.target.value)}
						onKeyDown={(e) => {
							if (e.key === "Enter") void submit();
						}}
					/>
					<p className="mono-label truncate text-[var(--color-ink-3)]">
						→ {target || "…"}
					</p>
				</div>

				<footer className="flex justify-end gap-2">
					<Button variant="outline" onClick={() => onOpenChange(false)}>
						取消
					</Button>
					<Button onClick={() => void submit()} disabled={busy}>
						{busy ? "移动中…" : "移动"}
					</Button>
				</footer>
			</DialogContent>
		</Dialog>
	);
}

function MoveRow({
	entry,
	depth,
	source,
	isDirMove,
	entries,
	expanded,
	selectedDir,
	onToggle,
	onSelect,
}: {
	entry: TreeEntry;
	depth: number;
	source: string;
	isDirMove: boolean;
	entries: Map<string, TreeEntry[]>;
	expanded: Set<string>;
	selectedDir: string;
	onToggle: (dir: string) => void;
	onSelect: (dir: string) => void;
}) {
	const isFolder = entry.type === "tree";
	const disabled = isDirMove && isWithin(source, entry.path);
	const isExpanded = expanded.has(entry.path);
	const children = entries.get(entry.path);
	const indent = { paddingLeft: `${depth * 16 + 12}px` };
	const rowSelected = !disabled && isFolder && selectedDir === entry.path;

	return (
		<div>
			<div
				role="button"
				tabIndex={0}
				aria-selected={rowSelected}
				aria-disabled={disabled}
				onClick={() => {
					if (isFolder && !disabled) onSelect(entry.path);
				}}
				onKeyDown={(e) => {
					if (e.key === "Enter" && isFolder && !disabled) onSelect(entry.path);
				}}
				className={`flex w-full items-center gap-1.5 py-1.5 pr-2 text-sm ${
					disabled
						? "cursor-not-allowed opacity-50"
						: rowSelected
							? "bg-[var(--color-surface-accent)]"
							: "hover:bg-[var(--color-surface-accent)]"
				}`}
				style={indent}
			>
				{isFolder ? (
					<button
						type="button"
						aria-label={isExpanded ? "收起" : "展开"}
						onClick={(e) => {
							e.stopPropagation();
							void onToggle(entry.path);
						}}
						className="flex size-4 shrink-0 items-center justify-center rounded-sm text-[var(--color-ink-3)] hover:text-[var(--color-ink)]"
					>
						<ChevronRight
							className={`size-3.5 transition-transform ${
								isExpanded ? "rotate-90" : ""
							}`}
						/>
					</button>
				) : (
					<span className="size-4 shrink-0" />
				)}
				{isFolder ? (
					<Folder className="size-4 shrink-0 text-[var(--color-accent)]" />
				) : (
					<FileText className="size-4 shrink-0 text-[var(--color-ink-3)]" />
				)}
				<span className="truncate text-[var(--color-ink)]">{entry.name}</span>
			</div>
			{isExpanded && (
				<div>
					{children ? (
						children.map((child) => (
							<MoveRow
								key={child.path}
								entry={child}
								depth={depth + 1}
								source={source}
								isDirMove={isDirMove}
								entries={entries}
								expanded={expanded}
								selectedDir={selectedDir}
								onToggle={onToggle}
								onSelect={onSelect}
							/>
						))
					) : (
						<div className="py-1 pl-8 text-xs text-[var(--color-ink-3)]">
							加载中…
						</div>
					)}
				</div>
			)}
		</div>
	);
}
