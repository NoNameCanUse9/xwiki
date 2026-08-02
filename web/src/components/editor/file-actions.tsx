import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { FilePlus2, Pencil, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  getRevision,
  submitChangeset,
  type ChangeInput,
} from "@/lib/api/changesets";

function pathValid(p: string): boolean {
  return /^[a-zA-Z0-9_\-\u4e00-\u9fa5/]+\.md$/.test(p) && !p.startsWith("/") && !p.includes("..");
}

export function NewPageForm({ projectId }: { projectId: string }) {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const [path, setPath] = useState("");
  const [busy, setBusy] = useState(false);

  const create = async () => {
    const p = path.trim();
    if (!pathValid(p)) {
      toast.error("路径需为 docs/xxx.md 形式（字母数字/下划线/连字符/中文/斜杠）");
      return;
    }
    setBusy(true);
    try {
      const rev = await getRevision(projectId);
      await submitChangeset(projectId, {
        base_revision: rev.revision,
        message: "", // 后端默认：时间 + 操作者 修改
        changes: [{ op: "create", path: p, content: `# ${p.split("/").pop()?.replace(/\.md$/, "")}\n\n` }],
      });
      toast.success(`已创建 ${p}`);
      setPath("");
      setOpen(false);
      await queryClient.invalidateQueries({ queryKey: ["tree"] });
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "创建失败");
    } finally {
      setBusy(false);
    }
  };

  if (!open) {
    return (
      <Button variant="ghost" size="sm" className="w-full justify-start gap-2 text-[var(--color-ink-3)]" onClick={() => setOpen(true)}>
        <FilePlus2 className="size-3.5" />
        新建页面
      </Button>
    );
  }
  return (
    <div className="space-y-2 p-2">
      <Input
        aria-label="新页面路径"
        placeholder="docs/hello.md"
        value={path}
        onChange={(e) => setPath(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") void create();
          if (e.key === "Escape") setOpen(false);
        }}
        className="h-8 font-mono text-xs"
      />
      <div className="flex gap-2">
        <Button size="sm" className="flex-1" onClick={() => void create()} disabled={busy}>
          {busy ? "创建中…" : "创建"}
        </Button>
        <Button size="sm" variant="outline" onClick={() => setOpen(false)}>
          取消
        </Button>
      </div>
    </div>
  );
}

export function FileRowActions({
  projectId,
  path,
  onDeleted,
}: {
  projectId: string;
  path: string;
  onDeleted: () => void;
}) {
  const queryClient = useQueryClient();
  const [renaming, setRenaming] = useState(false);
  const [newPath, setNewPath] = useState(path);
  const [busy, setBusy] = useState(false);

  const run = async (changes: ChangeInput[], successMsg: string) => {
    setBusy(true);
    try {
      const rev = await getRevision(projectId);
      await submitChangeset(projectId, {
        base_revision: rev.revision,
        message: "",
        changes,
      });
      toast.success(successMsg);
      await queryClient.invalidateQueries({ queryKey: ["tree"] });
      await queryClient.invalidateQueries({ queryKey: ["docs"] });
      onDeleted();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "操作失败");
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (!window.confirm(`确认删除 ${path}？将创建一个删除提交。`)) return;
    await run([{ op: "delete", path }], `已删除 ${path}`);
  };

  const rename = async () => {
    const p = newPath.trim();
    if (!pathValid(p)) {
      toast.error("路径需为 docs/xxx.md 形式");
      return;
    }
    if (p === path) {
      setRenaming(false);
      return;
    }
    await run([{ op: "move", path, new_path: p }], `已重命名 → ${p}`);
    setRenaming(false);
  };

  if (renaming) {
    return (
      <span className="flex items-center gap-1 px-1">
        <Input
          aria-label="重命名路径"
          value={newPath}
          onChange={(e) => setNewPath(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void rename();
            if (e.key === "Escape") setRenaming(false);
          }}
          className="h-6 w-40 font-mono text-xs"
        />
        <Button size="sm" variant="outline" onClick={() => void rename()} disabled={busy}>
          ✓
        </Button>
      </span>
    );
  }
  return (
    <span className="flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
      <Button
        variant="ghost"
        size="sm"
        className="h-6 w-6 p-0 text-[var(--color-ink-3)]"
        aria-label={`重命名 ${path}`}
        title="重命名"
        onClick={() => {
          setNewPath(path);
          setRenaming(true);
        }}
      >
        <Pencil className="size-3" />
      </Button>
      <Button
        variant="ghost"
        size="sm"
        className="h-6 w-6 p-0 text-[var(--color-destructive)]"
        aria-label={`删除 ${path}`}
        title="删除"
        onClick={() => void remove()}
        disabled={busy}
      >
        <Trash2 className="size-3" />
      </Button>
    </span>
  );
}
