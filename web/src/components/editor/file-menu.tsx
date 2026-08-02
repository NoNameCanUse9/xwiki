import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import {
  Archive,
  CornerDownRight,
  Edit3,
  Ellipsis,
  History,
  Link2,
  Paperclip,
  Pencil,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import {
  getRevision,
  submitChangeset,
  type ChangeInput,
} from "@/lib/api/changesets";

export interface FileMenuItems {
  onEdit?: () => void;
  onToggleHistory?: () => void;
  onToggleAttachments?: () => void;
  onToggleBacklinks?: () => void;
}

/**
 * GitHub-style "⋯" file menu: edit, history, copy link/path, rename, delete,
 * and toggles for attachments / backlinks panels.
 */
export default function FileMenu({
  projectId,
  filePath,
  items,
}: {
  projectId: string;
  filePath: string;
  items: FileMenuItems;
}) {
  const [open, setOpen] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [newPath, setNewPath] = useState(filePath);
  const [busy, setBusy] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const queryClient = useQueryClient();
  const navigate = useNavigate();

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  const close = () => setOpen(false);

  const runChanges = async (
    changes: ChangeInput[],
    successMsg: string,
  ) => {
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
      setOpen(false);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "操作失败");
    } finally {
      setBusy(false);
    }
  };

  const copyLink = async () => {
    try {
      await navigator.clipboard.writeText(window.location.href);
      toast.success("已复制链接");
    } catch {
      toast.error("复制失败");
    }
    close();
  };

  const copyPath = async () => {
    try {
      await navigator.clipboard.writeText(filePath);
      toast.success("已复制路径");
    } catch {
      toast.error("复制失败");
    }
    close();
  };

  const remove = async () => {
    if (!window.confirm(`确认删除 ${filePath}？将创建一个删除提交。`)) return;
    await runChanges([{ op: "delete", path: filePath }], `已删除 ${filePath}`);
    navigate(`/projects/${projectId}/docs`);
  };

  const rename = async () => {
    const p = newPath.trim();
    if (!p || p === filePath) {
      setRenaming(false);
      close();
      return;
    }
    await runChanges(
      [{ op: "move", path: filePath, new_path: p }],
      `已重命名 → ${p}`,
    );
    setRenaming(false);
    navigate(`/projects/${projectId}/docs/${p}`);
  };

  const itemCls =
    "flex w-full items-center gap-2 rounded-sm px-3 py-1.5 text-left text-sm text-[var(--color-ink-2)] hover:bg-[var(--color-surface-accent)] hover:text-[var(--color-ink)]";

  return (
    <div ref={menuRef} className="relative">
      <button
        type="button"
        aria-label="文件操作"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="flex h-8 w-8 items-center justify-center rounded-sm text-[var(--color-ink-3)] hover:bg-[var(--color-surface-accent)] hover:text-[var(--color-ink)]"
      >
        <Ellipsis className="size-4" />
      </button>
      {open && (
        <div
          role="menu"
          className="absolute right-0 top-9 z-30 w-52 rounded-[var(--radius)] border border-[var(--color-rule)] bg-[var(--color-paper)] p-1 shadow-lg"
        >
          {items.onEdit && (
            <button type="button" role="menuitem" className={itemCls} onClick={() => { items.onEdit?.(); close(); }}>
              <Edit3 className="size-3.5" /> 编辑
            </button>
          )}
          {items.onToggleHistory && (
            <button type="button" role="menuitem" className={itemCls} onClick={() => { items.onToggleHistory?.(); close(); }}>
              <History className="size-3.5" /> 历史版本
            </button>
          )}
          {items.onToggleAttachments && (
            <button type="button" role="menuitem" className={itemCls} onClick={() => { items.onToggleAttachments?.(); close(); }}>
              <Paperclip className="size-3.5" /> 附件
            </button>
          )}
          {items.onToggleBacklinks && (
            <button type="button" role="menuitem" className={itemCls} onClick={() => { items.onToggleBacklinks?.(); close(); }}>
              <CornerDownRight className="size-3.5" /> 反向链接
            </button>
          )}
          <div className="my-1 h-px bg-[var(--color-rule)]" />
          <button type="button" role="menuitem" className={itemCls} onClick={() => void copyLink()}>
            <Link2 className="size-3.5" /> 复制链接
          </button>
          <button type="button" role="menuitem" className={itemCls} onClick={() => void copyPath()}>
            <Archive className="size-3.5" /> 复制路径
          </button>
          <div className="my-1 h-px bg-[var(--color-rule)]" />
          {renaming ? (
            <div className="space-y-1 p-2">
              <input
                aria-label="重命名路径"
                value={newPath}
                onChange={(e) => setNewPath(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void rename();
                  if (e.key === "Escape") setRenaming(false);
                }}
                className="h-7 w-full rounded-[var(--radius)] border border-[var(--color-rule)] bg-[var(--color-paper)] px-2 font-mono text-xs text-[var(--color-ink)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]"
              />
              <div className="flex gap-1">
                <button
                  type="button"
                  className="flex-1 rounded-sm bg-[var(--color-accent)] px-2 py-1 text-xs text-[var(--color-accent-ink)] disabled:opacity-50"
                  disabled={busy}
                  onClick={() => void rename()}
                >
                  确定
                </button>
                <button
                  type="button"
                  className="rounded-sm border border-[var(--color-rule)] px-2 py-1 text-xs text-[var(--color-ink-2)]"
                  onClick={() => setRenaming(false)}
                >
                  取消
                </button>
              </div>
            </div>
          ) : (
            <button
              type="button"
              role="menuitem"
              className={itemCls}
              onClick={() => {
                setNewPath(filePath);
                setRenaming(true);
              }}
            >
              <Pencil className="size-3.5" /> 重命名
            </button>
          )}
          <button
            type="button"
            role="menuitem"
            className={itemCls + " text-[var(--color-destructive)]"}
            disabled={busy}
            onClick={() => void remove()}
          >
            <Trash2 className="size-3.5" /> 删除
          </button>
        </div>
      )}
    </div>
  );
}
