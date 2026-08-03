import { useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { FileUp, FolderUp } from "lucide-react";
import { toast } from "sonner";
import JSZip from "jszip";
import { getRevision } from "@/lib/api/changesets";
import { importZip, type ImportFile } from "@/lib/api/transfer";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogHeader,
	DialogTitle,
	DialogTrigger,
} from "@/components/ui/dialog";

const MAX_TOTAL = 20 * 1024 * 1024; // 20 MiB per import batch
const MAX_FILE = 5 * 1024 * 1024;

/**
 * Import files/folders into the project: either a ZIP archive or a folder
 * selection (webkitdirectory). Everything lands in one changeset commit.
 */
export default function ImportFilesButton({
	projectId,
}: {
	projectId: string;
}) {
	const queryClient = useQueryClient();
	const fileRef = useRef<HTMLInputElement>(null);
	const dirRef = useRef<HTMLInputElement>(null);
	const [busy, setBusy] = useState(false);
	const [open, setOpen] = useState(false);

	const runImport = async (files: ImportFile[]) => {
		if (files.length === 0) {
			toast.error("没有可导入的文件");
			return;
		}
		setBusy(true);
		try {
			const rev = await getRevision(projectId);
			const res = await importZip(projectId, rev.revision, files);
			toast.success(`已导入 ${res.imported} 个文件`);
			await queryClient.invalidateQueries({ queryKey: ["tree"] });
			await queryClient.invalidateQueries({ queryKey: ["dir"] });
			await queryClient.invalidateQueries({ queryKey: ["page-index"] });
		} catch (err) {
			toast.error(err instanceof Error ? err.message : "导入失败");
		} finally {
			setBusy(false);
			setOpen(false);
			if (fileRef.current) fileRef.current.value = "";
			if (dirRef.current) dirRef.current.value = "";
		}
	};

	const readAsBase64 = (f: File): Promise<string> =>
		new Promise((resolve, reject) => {
			const reader = new FileReader();
			reader.onload = () => {
				const r = reader.result as string;
				resolve(r.slice(r.indexOf(",") + 1));
			};
			reader.onerror = () => reject(new Error("读取失败"));
			reader.readAsDataURL(f);
		});

	const handleZip = async (f: File) => {
		if (f.size > MAX_TOTAL) {
			toast.error("ZIP 超过 20 MiB");
			return;
		}
		try {
			const zip = await JSZip.loadAsync(f);
			const files: ImportFile[] = [];
			let total = 0;
			for (const [path, entry] of Object.entries(zip.files)) {
				if (entry.dir) continue;
				if (total > MAX_TOTAL) break;
				const content = await entry.async("base64");
				const size = (content.length * 3) / 4;
				if (size > MAX_FILE) continue;
				total += size;
				files.push({ path, content });
			}
			await runImport(files);
		} catch {
			toast.error("ZIP 解析失败");
		}
	};

	const handleFiles = async (list: FileList) => {
		const files: ImportFile[] = [];
		let total = 0;
		for (const f of Array.from(list)) {
			if (f.size > MAX_FILE) continue;
			total += f.size;
			if (total > MAX_TOTAL) break;
			files.push({
				path: f.webkitRelativePath || f.name,
				content: await readAsBase64(f),
			});
		}
		await runImport(files);
	};

	return (
		<>
			<input
				ref={fileRef}
				type="file"
				accept=".zip"
				aria-label="导入 ZIP"
				className="hidden"
				onChange={(e) => {
					const f = e.target.files?.[0];
					if (f) void handleZip(f);
				}}
			/>
			<input
				ref={dirRef}
				type="file"
				aria-label="导入文件夹"
				className="hidden"
				// @ts-expect-error webkitdirectory is a non-standard attribute
				webkitdirectory=""
				multiple
				onChange={(e) => {
					if (e.target.files && e.target.files.length > 0)
						void handleFiles(e.target.files);
				}}
			/>
			<Dialog open={open} onOpenChange={setOpen}>
				<DialogTrigger asChild>
					<Button variant="outline" size="sm" className="gap-2">
						<FileUp className="size-3.5" />
						导入
					</Button>
				</DialogTrigger>
				<DialogContent>
					<DialogHeader>
						<DialogTitle>导入文件</DialogTitle>
						<DialogDescription>
							导入 ZIP 或文件夹到当前项目，全部文件一次提交。
						</DialogDescription>
					</DialogHeader>
					<div className="grid gap-2">
						<Button
							variant="outline"
							className="gap-2"
							disabled={busy}
							onClick={() => fileRef.current?.click()}
						>
							<FileUp className="size-4" />
							{busy ? "导入中…" : "导入 ZIP 文件"}
						</Button>
						<Button
							variant="outline"
							className="gap-2"
							disabled={busy}
							onClick={() => dirRef.current?.click()}
						>
							<FolderUp className="size-4" />
							{busy ? "导入中…" : "导入文件夹"}
						</Button>
					</div>
				</DialogContent>
			</Dialog>
		</>
	);
}
