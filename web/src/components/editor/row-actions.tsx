import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
	FileText,
	FolderInput,
	FolderPlus,
	MoreHorizontal,
	Pencil,
	Trash2,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import MoveDialog from "@/components/editor/move-dialog";
import {
	getRevision,
	submitChangeset,
	type ChangeInput,
} from "@/lib/api/changesets";

function pathValid(p: string): boolean {
	return (
		/^[a-zA-Z0-9_\-\u4e00-\u9fa5/]+(\.md)?$/.test(p) &&
		!p.startsWith("/") &&
		!p.includes("..")
	);
}

interface RowActionsProps {
	projectId: string;
	path: string;
	type: "file" | "folder";
	onDone: () => void;
	onOpen?: () => void;
}

export default function RowActions({
	projectId,
	path,
	type,
	onDone,
	onOpen,
}: RowActionsProps) {
	const queryClient = useQueryClient();
	const [open, setOpen] = useState(false);
	const [renaming, setRenaming] = useState(false);
	const [newPath, setNewPath] = useState(path);
	const [busy, setBusy] = useState(false);
	const [creatingSubfolder, setCreatingSubfolder] = useState(false);
	const [subfolderName, setSubfolderName] = useState("");
	const [moveOpen, setMoveOpen] = useState(false);

	const run = async (changes: ChangeInput[], msg: string) => {
		setBusy(true);
		try {
			const rev = await getRevision(projectId);
			await submitChangeset(projectId, {
				base_revision: rev.revision,
				message: "",
				changes,
			});
			toast.success(msg);
			setOpen(false);
			await queryClient.invalidateQueries({ queryKey: ["tree"] });
			await queryClient.invalidateQueries({ queryKey: ["dir"] });
			onDone();
		} catch (err) {
			toast.error(err instanceof Error ? err.message : "操作失败");
		} finally {
			setBusy(false);
		}
	};

	const remove = async () => {
		if (!window.confirm(`确认删除 ${path}？`)) return;
		await run([{ op: "delete", path }], `已删除 ${path}`);
	};

	const rename = async () => {
		const p = newPath.trim();
		if (!pathValid(p)) {
			toast.error("路径不合法");
			return;
		}
		if (p === path) {
			setRenaming(false);
			return;
		}
		await run([{ op: "move", path, new_path: p }], `已重命名 → ${p}`);
		setRenaming(false);
	};

	const createSubfolder = async () => {
		const name = subfolderName.trim();
		if (!name) {
			toast.error("请输入名称");
			return;
		}
		const folderPath =
			path === "" ? `${name}/.gitkeep` : `${path}/${name}/.gitkeep`;
		await run(
			[{ op: "create", path: folderPath, content: "" }],
			`已创建 ${name}/`,
		);
		setCreatingSubfolder(false);
		setSubfolderName("");
	};

	if (renaming) {
		return (
			<span className="flex items-center gap-1 px-1">
				<Input
					aria-label="重命名"
					value={newPath}
					onChange={(e) => setNewPath(e.target.value)}
					onKeyDown={(e) => {
						if (e.key === "Enter") void rename();
						if (e.key === "Escape") setRenaming(false);
					}}
					className="h-6 w-40 font-mono text-xs"
					autoFocus
				/>
				<Button
					size="sm"
					variant="outline"
					onClick={() => void rename()}
					disabled={busy}
				>
					✓
				</Button>
			</span>
		);
	}

	if (creatingSubfolder) {
		return (
			<span className="flex items-center gap-1 px-1">
				<Input
					aria-label="子文件夹名称"
					placeholder="文件夹名"
					value={subfolderName}
					onChange={(e) => setSubfolderName(e.target.value)}
					onKeyDown={(e) => {
						if (e.key === "Enter") void createSubfolder();
						if (e.key === "Escape") setCreatingSubfolder(false);
					}}
					className="h-6 w-32 text-xs"
					autoFocus
				/>
				<Button
					size="sm"
					variant="outline"
					onClick={() => void createSubfolder()}
					disabled={busy}
				>
					✓
				</Button>
			</span>
		);
	}

	return (
		<span className="relative" onClick={(e) => e.stopPropagation()}>
			<button
				type="button"
				onClick={() => setOpen((v) => !v)}
				className="flex h-6 w-6 items-center justify-center rounded-sm text-[var(--color-ink-3)] hover:bg-[var(--color-surface-accent)] hover:text-[var(--color-ink)]"
			>
				<MoreHorizontal className="size-4" />
			</button>
			{open && (
				<>
					<div className="fixed inset-0 z-40" onClick={() => setOpen(false)} />
					<div className="absolute right-0 top-full z-50 mt-1 min-w-[140px] rounded-[var(--radius)] border border-[var(--color-rule)] bg-[var(--color-paper)] py-1 shadow-lg">
						{type === "file" && onOpen && (
							<button
								type="button"
								className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm hover:bg-[var(--color-surface-accent)]"
								onClick={() => {
									setOpen(false);
									onOpen();
								}}
							>
								<FileText className="size-3.5" />
								编辑
							</button>
						)}
						{type === "folder" && (
							<button
								type="button"
								className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm hover:bg-[var(--color-surface-accent)]"
								onClick={() => {
									setOpen(false);
									setCreatingSubfolder(true);
								}}
							>
								<FolderPlus className="size-3.5" />
								新建子文件夹
							</button>
						)}
						<button
							type="button"
							className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm hover:bg-[var(--color-surface-accent)]"
							onClick={() => {
								setOpen(false);
								setNewPath(path);
								setRenaming(true);
							}}
						>
							<Pencil className="size-3.5" />
							重命名
						</button>
						<button
							type="button"
							className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm hover:bg-[var(--color-surface-accent)]"
							onClick={() => {
								setOpen(false);
								setMoveOpen(true);
							}}
						>
							<FolderInput className="size-3.5" />
							移动
						</button>
						<button
							type="button"
							className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm text-[var(--color-destructive)] hover:bg-[var(--color-surface-accent)]"
							onClick={() => void remove()}
							disabled={busy}
						>
							<Trash2 className="size-3.5" />
							删除
						</button>
					</div>
				</>
			)}
			<MoveDialog
				projectId={projectId}
				source={path}
				isDir={type === "folder"}
				open={moveOpen}
				onOpenChange={setMoveOpen}
				onDone={onDone}
			/>
		</span>
	);
}
