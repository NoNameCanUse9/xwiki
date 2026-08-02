import { useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "react-router-dom";
import { ArrowLeft, ChevronRight, FileText, Folder, FolderOpen, Pencil, RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { getRevision, submitChangeset } from "@/lib/api/changesets";
import { getHome, getPage, getTree, type TreeEntry } from "@/lib/api/docs";

function dirOf(filePath: string): string {
  const i = filePath.lastIndexOf("/");
  return i >= 0 ? filePath.slice(0, i) : "";
}

function sanitizeHtml(html: string): string {
  return html
    .replace(/<script[\s\S]*?<\/script>/gi, "")
    .replace(/<iframe[\s\S]*?<\/iframe>/gi, "")
    .replace(/on\w+="[^"]*"/g, "")
    .replace(/on\w+='[^']*'/g, "");
}

function Breadcrumbs({ projectId, filePath }: { projectId: string; filePath: string }) {
  const segments = filePath.split("/").filter(Boolean);
  return (
    <nav aria-label="面包屑" className="mono-label flex flex-wrap items-center gap-1 text-[var(--color-ink-3)]">
      <Link to={`/projects/${projectId}/docs`} className="hover:text-[var(--color-accent)]">
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
              <Link to={`/projects/${projectId}/docs/${prefix}`} className="hover:text-[var(--color-accent)]">
                {seg}
              </Link>
            )}
          </span>
        );
      })}
    </nav>
  );
}

interface DirNodeProps {
  projectId: string;
  dir: string;
  depth: number;
  expandedDirs: Set<string>;
  onToggle: (entry: TreeEntry) => void;
  onOpen: (entry: TreeEntry) => void;
}

function DirNode({ projectId, dir, depth, expandedDirs, onToggle, onOpen }: DirNodeProps) {
  const { data } = useQuery({
    queryKey: ["tree", projectId, dir],
    queryFn: () => getTree(projectId, dir),
    enabled: depth === 0 || expandedDirs.has(dir),
  });

  if (depth > 0 && !expandedDirs.has(dir)) return null;

  return (
    <div>
      {!data && (
        <p className="mono-label px-2 py-1 text-[var(--color-ink-3)]">loading…</p>
      )}
      {data?.tree.map((entry) => (
        <div key={entry.path}>
          <button
            type="button"
            onClick={() => (entry.type === "tree" ? onToggle(entry) : onOpen(entry))}
            className="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm text-[var(--color-ink-2)] hover:bg-[var(--color-surface-accent)] hover:text-[var(--color-ink)]"
            style={{ paddingLeft: `${8 + depth * 14}px` }}
            title={entry.path}
          >
            {entry.type === "tree" ? (
              expandedDirs.has(entry.path) ? (
                <FolderOpen className="size-3.5 shrink-0 text-[var(--color-accent)]" />
              ) : (
                <Folder className="size-3.5 shrink-0 text-[var(--color-accent)]" />
              )
            ) : (
              <FileText className="size-3.5 shrink-0 text-[var(--color-ink-3)]" />
            )}
            <span className="truncate">{entry.name}</span>
          </button>
          {entry.type === "tree" && (
            <DirNode
              projectId={projectId}
              dir={entry.path}
              depth={depth + 1}
              expandedDirs={expandedDirs}
              onToggle={onToggle}
              onOpen={onOpen}
            />
          )}
        </div>
      ))}
    </div>
  );
}

export default function DocsViewerPage() {
  const { id = "", "*": filePath = "" } = useParams();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [expandedDirs, setExpandedDirs] = useState<Set<string>>(new Set());
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [saving, setSaving] = useState(false);

  const showHome = !filePath;

  // Auto-expand the chain of directories leading to the current file.
  const dirsToLoad = useMemo(() => {
    const dirs: string[] = [];
    let cur = dirOf(filePath);
    while (cur) {
      dirs.unshift(cur);
      cur = dirOf(cur);
    }
    return dirs;
  }, [filePath]);

  useEffect(() => {
    setExpandedDirs((prev) => {
      const next = new Set(prev);
      dirsToLoad.forEach((d) => next.add(d));
      return next;
    });
  }, [dirsToLoad]);

  const homeQuery = useQuery({
    queryKey: ["docs", "home", id],
    queryFn: () => getHome(id),
    enabled: showHome,
  });

  const pageQuery = useQuery({
    queryKey: ["docs", "page", id, filePath],
    queryFn: () => getPage(id, filePath),
    enabled: !showHome,
  });

  const toggleDir = (entry: TreeEntry) => {
    setExpandedDirs((prev) => {
      const next = new Set(prev);
      if (next.has(entry.path)) next.delete(entry.path);
      else next.add(entry.path);
      return next;
    });
  };

  const openEntry = (entry: TreeEntry) => {
    if (entry.type === "blob") {
      navigate(`/projects/${id}/docs/${entry.path}`);
    } else {
      toggleDir(entry);
    }
  };

  // Edit flow: load raw content, submit an update changeset on save.
  const rawQuery = useQuery({
    queryKey: ["docs", "raw", id, filePath],
    queryFn: () => getPage(id, filePath, "raw"),
    enabled: editing && !showHome,
  });

  useEffect(() => {
    if (rawQuery.data && draft === "") {
      setDraft(rawQuery.data.content);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rawQuery.data]);

  const startEdit = () => {
    setDraft("");
    setEditing(true);
  };

  const saveEdit = async () => {
    setSaving(true);
    try {
      const rev = await getRevision(id);
      await submitChangeset(id, {
        base_revision: rev.revision,
        message: `Update ${filePath}`,
        changes: [{ op: "update", path: filePath, content: draft }],
      });
      toast.success("已保存");
      setEditing(false);
      await queryClient.invalidateQueries({ queryKey: ["docs"] });
      await queryClient.invalidateQueries({ queryKey: ["tree"] });
    } catch (err) {
      if ((err as { status?: number })?.status === 409) {
        toast.error("文档已被他人修改，请刷新后重试");
        setEditing(false);
      } else {
        toast.error(err instanceof Error ? err.message : "保存失败");
      }
    } finally {
      setSaving(false);
    }
  };

  const content = showHome ? homeQuery.data : pageQuery.data;
  const loading = showHome ? homeQuery.isLoading : pageQuery.isLoading;
  const error = showHome ? homeQuery.isError : pageQuery.isError;

  return (
    <div className="flex min-h-screen">
      <aside className="hidden w-64 shrink-0 flex-col border-r border-[var(--color-rule)] bg-[var(--color-paper-2)] sm:flex">
        <div className="border-b border-[var(--color-rule)] px-4 py-3">
          <Link
            to="/"
            className="mono-label flex items-center gap-2 text-[var(--color-ink-3)] hover:text-[var(--color-accent)]"
          >
            <ArrowLeft className="size-3.5" />
            workspace
          </Link>
        </div>
        <div className="flex-1 overflow-y-auto p-2">
          <DirNode
            projectId={id}
            dir=""
            depth={0}
            expandedDirs={expandedDirs}
            onToggle={toggleDir}
            onOpen={openEntry}
          />
        </div>
      </aside>

      <div className="flex w-full flex-col">
        <header className="flex items-center justify-between border-b border-[var(--color-rule)] px-6 py-3">
          <Breadcrumbs projectId={id} filePath={filePath} />
        </header>

        <main className="flex-1 px-6 py-8 sm:px-10">
          <div className="mx-auto w-full max-w-3xl">
            {loading && <p className="mono-label text-[var(--color-ink-3)]">loading…</p>}
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
            {content && content.format === "html" && !editing && (
              <article
                className="prose-agentdocs"
                dangerouslySetInnerHTML={{ __html: sanitizeHtml(content.content) }}
              />
            )}
            {content && content.format === "raw" && !editing && (
              <pre className="code-card overflow-x-auto p-4">{content.content}</pre>
            )}
            {!showHome && !editing && (
              <div className="mt-6 flex items-center gap-3">
                <Button variant="outline" size="sm" className="gap-2" onClick={startEdit}>
                  <Pencil className="size-3.5" />
                  编辑
                </Button>
                <span className="mono-label text-[var(--color-ink-3)]">
                  {filePath}
                </span>
              </div>
            )}
            {editing && (
              <div className="space-y-3">
                <div className="flex items-center justify-between">
                  <p className="mono-label text-[var(--color-ink-3)]">
                    editing · {filePath}
                  </p>
                  <Button variant="ghost" size="sm" onClick={() => setEditing(false)}>
                    <RefreshCw className="mr-1.5 size-3.5" />
                    取消
                  </Button>
                </div>
                {rawQuery.isLoading ? (
                  <p className="mono-label text-[var(--color-ink-3)]">loading…</p>
                ) : (
                  <textarea
                    aria-label="文档内容"
                    value={draft}
                    onChange={(e) => setDraft(e.target.value)}
                    rows={22}
                    spellCheck={false}
                    className="code-card w-full resize-y p-4 font-mono text-sm leading-relaxed outline-none focus:border-[var(--color-accent)]"
                  />
                )}
                <div className="flex justify-end gap-2">
                  <Button variant="outline" size="sm" onClick={() => setEditing(false)}>
                    放弃
                  </Button>
                  <Button size="sm" onClick={() => void saveEdit()} disabled={saving}>
                    {saving ? "保存中…" : "保存"}
                  </Button>
                </div>
              </div>
            )}
          </div>
        </main>
      </div>
    </div>
  );
}
