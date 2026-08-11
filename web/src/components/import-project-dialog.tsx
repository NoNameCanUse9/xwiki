import { useState } from "react";
import { useNavigate } from "react-router-dom";
import JSZip from "jszip";
import { FolderUp, Upload } from "lucide-react";
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
  DialogTrigger,
} from "@/components/ui/dialog";
import { importFolder } from "@/lib/api/transfer";

/** Import a project from a local folder. */
export default function ImportProjectDialog() {
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [folderFiles, setFolderFiles] = useState<File[]>([]);
  const [busy, setBusy] = useState(false);

  const reset = () => {
    setName("");
    setFolderFiles([]);
  };

  const handleFolder = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (!files || files.length === 0) return;
    setFolderFiles(Array.from(files));
    // Derive project name from the first file's directory
    const first = files[0];
    const dir = first.webkitRelativePath?.split("/")[0];
    if (dir && !name)
      setName(
        dir
          .toLowerCase()
          .replace(/[^a-z0-9]+/g, "-")
          .replace(/^-|-$/g, ""),
      );
  };

  const handleZip = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    setBusy(true);
    try {
      const zip = await JSZip.loadAsync(file);
      const entries = Object.values(zip.files).filter((entry) => !entry.dir);
      if (entries.length === 0) {
        toast.error("zip 中没有文件");
        setFolderFiles([]);
        return;
      }
      // 若所有条目共享同一顶层目录则剥离它，保持与文件夹导入一致。
      const tops = new Set(
        entries.map((entry) => entry.name.split("/")[0]).filter(Boolean),
      );
      const stripTop = tops.size === 1 ? [...tops][0] : "";
      const files: File[] = [];
      for (const entry of entries) {
        const rel = stripTop
          ? entry.name.slice(stripTop.length + 1)
          : entry.name;
        const blob = await entry.async("blob");
        const f = new File([blob], entry.name.split("/").pop() ?? rel, {
          type: "application/octet-stream",
        });
        Object.defineProperty(f, "__relPath", {
          value: rel,
          writable: true,
        });
        files.push(f);
      }
      setFolderFiles(files);
      if (!name) {
        setName(
          file.name
            .replace(/\.zip$/i, "")
            .toLowerCase()
            .replace(/[^a-z0-9]+/g, "-")
            .replace(/^-|-$/g, ""),
        );
      }
    } catch {
      toast.error("无法解析 zip 文件");
      setFolderFiles([]);
    } finally {
      setBusy(false);
    }
  };

  const submit = async () => {
    const n = name.trim();
    if (!n) {
      toast.error("项目名必填");
      return;
    }
    if (folderFiles.length === 0) {
      toast.error("请选择文件夹");
      return;
    }
    setBusy(true);
    try {
      const res = await importFolder(n, "", folderFiles);
      toast.success(`已导入 ${res.project.name}（${res.commits} 个提交）`);
      setOpen(false);
      reset();
      navigate(`/projects/${res.project.id}/docs`);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "导入失败");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(v) => {
        setOpen(v);
        if (!v) reset();
      }}
    >
      <DialogTrigger asChild>
        <Button variant="outline" className="gap-2">
          <FolderUp className="size-4" />
          导入项目
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>导入项目</DialogTitle>
          <DialogDescription>
            上传本地文件夹创建新项目；文件夹含 .git/ 则保留历史，否则自动 git
            init 并提交全部文件。
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="imp-name">项目名</Label>
            <Input
              id="imp-name"
              placeholder="my-docs"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>
          <div className="space-y-2">
            {folderFiles.length === 0 ? (
              <>
                <Label htmlFor="imp-folder">选择文件夹</Label>
                <Input
                  id="imp-folder"
                  type="file"
                  // @ts-expect-error — webkitdirectory is non-standard but supported by all browsers
                  webkitdirectory=""
                  directory=""
                  multiple
                  onChange={handleFolder}
                  className="cursor-pointer"
                />
                <div className="flex items-center gap-2">
                  <span className="h-px flex-1 bg-[var(--color-rule)]" />
                  <span className="mono-label text-[var(--color-ink-3)]">or</span>
                  <span className="h-px flex-1 bg-[var(--color-rule)]" />
                </div>
                <Label htmlFor="imp-zip">选择 zip 文件</Label>
                <Input
                  id="imp-zip"
                  type="file"
                  accept=".zip,application/zip"
                  onChange={(e) => void handleZip(e)}
                  className="cursor-pointer"
                />
              </>
            ) : (
              <p className="text-xs text-[var(--color-ink-3)]">
                已选择 {folderFiles.length} 个文件，包含 .git/
                则保留历史，否则创建新仓库。
              </p>
            )}
          </div>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => setOpen(false)}>
              取消
            </Button>
            <Button
              onClick={() => void submit()}
              disabled={busy}
              className="gap-2"
            >
              <Upload className="size-4" />
              {busy ? "导入中…" : "导入"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
