import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { FolderUp, GitBranch, Upload } from "lucide-react";
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
import { importRepo } from "@/lib/api/transfer";
import { ApiError } from "@/lib/api/client";

/** Import a project from a remote git URL or a bundle/zip file. */
export default function ImportProjectDialog() {
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    const n = name.trim();
    const u = url.trim();
    if (!n || !u) {
      toast.error("项目名与 Git URL 必填");
      return;
    }
    setBusy(true);
    try {
      const res = await importRepo(n, u);
      toast.success(`已导入 ${res.project.name}（${res.commits} 个提交）`);
      setOpen(false);
      setName("");
      setUrl("");
      navigate(`/projects/${res.project.id}/docs`);
    } catch (err) {
      if (err instanceof ApiError && err.status === 400) {
        toast.error("URL 无效或仓库无法访问");
      } else {
        toast.error(err instanceof Error ? err.message : "导入失败");
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
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
            从远程 Git 仓库 URL 克隆创建新项目（完整历史保留）。
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
            <Label htmlFor="imp-url">Git 仓库 URL</Label>
            <Input
              id="imp-url"
              placeholder="https://github.com/user/repo.git"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void submit();
              }}
            />
          </div>
          <div className="flex items-center gap-2 rounded-[var(--radius)] border border-[var(--color-rule)] bg-[var(--color-paper-2)] px-3 py-2.5">
            <GitBranch className="size-4 text-[var(--color-ink-3)]" />
            <p className="text-xs text-[var(--color-ink-3)]">
              支持 http(s)://、git://、ssh:// 与 scp 风格地址
            </p>
          </div>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => setOpen(false)}>
              取消
            </Button>
            <Button onClick={() => void submit()} disabled={busy} className="gap-2">
              <Upload className="size-4" />
              {busy ? "克隆中…" : "导入"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
