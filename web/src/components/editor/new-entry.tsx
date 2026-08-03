import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { FilePlus2, FolderPlus } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { getRevision, submitChangeset } from "@/lib/api/changesets";

export default function NewEntryButton({ projectId }: { projectId: string }) {
	const queryClient = useQueryClient();
	const [open, setOpen] = useState(false);
	const [name, setName] = useState("");
	const [busy, setBusy] = useState(false);

	const create = async (type: "file" | "folder") => {
		const n = name.trim();
		if (!n) {
			toast.error("请输入名称");
			return;
		}
		setBusy(true);
		try {
			const rev = await getRevision(projectId);
			if (type === "folder") {
				await submitChangeset(projectId, {
					base_revision: rev.revision,
					message: "",
					changes: [{ op: "create", path: `${n}/.gitkeep`, content: "" }],
				});
				toast.success(`已创建文件夹 ${n}/`);
			} else {
				const mdName = n.endsWith(".md") ? n : n + ".md";
				const title = mdName.replace(/\.md$/, "").split("/").pop() || mdName;
				await submitChangeset(projectId, {
					base_revision: rev.revision,
					message: "",
					changes: [{ op: "create", path: mdName, content: `# ${title}\n\n` }],
				});
				toast.success(`已创建文件 ${mdName}`);
			}
			setName("");
			setOpen(false);
			await queryClient.invalidateQueries({ queryKey: ["tree"] });
			await queryClient.invalidateQueries({ queryKey: ["dir"] });
		} catch (err) {
			toast.error(err instanceof Error ? err.message : "创建失败");
		} finally {
			setBusy(false);
		}
	};

	return (
		<>
			<Button
				variant="outline"
				size="sm"
				className="gap-2"
				onClick={() => setOpen(true)}
			>
				<FilePlus2 className="size-4" />
				新建
			</Button>
			<Dialog
				open={open}
				onOpenChange={(v) => {
					setOpen(v);
					if (!v) setName("");
				}}
			>
				<DialogContent>
					<DialogHeader>
						<DialogTitle>新建</DialogTitle>
						<DialogDescription>
							输入名称，选择创建文件或文件夹。
						</DialogDescription>
					</DialogHeader>
					<div className="space-y-4">
						<div className="space-y-2">
							<Label htmlFor="new-name">名称</Label>
							<Input
								id="new-name"
								placeholder="hello 或 subfolder"
								value={name}
								onChange={(e) => setName(e.target.value)}
								onKeyDown={(e) => {
									if (e.key === "Enter") {
										e.preventDefault();
										void create("file");
									}
								}}
								autoFocus
							/>
						</div>
						<div className="flex justify-end gap-2">
							<Button variant="outline" onClick={() => setOpen(false)}>
								取消
							</Button>
							<Button
								onClick={() => void create("file")}
								disabled={busy || !name.trim()}
								className="gap-2"
							>
								<FilePlus2 className="size-4" />
								{busy ? "创建中…" : "新建 MD 文件"}
							</Button>
							<Button
								variant="secondary"
								onClick={() => void create("folder")}
								disabled={busy || !name.trim()}
								className="gap-2"
							>
								<FolderPlus className="size-4" />
								{busy ? "创建中…" : "新建文件夹"}
							</Button>
						</div>
					</div>
				</DialogContent>
			</Dialog>
		</>
	);
}
