import { useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "react-router-dom";
import { ArrowLeft, ChevronRight, FileText, Folder, FolderOpen, History, Pencil, RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { getRevision, submitChangeset } from "@/lib/api/changesets";
import { fileHistory } from "@/lib/api/history";
import { searchProject } from "@/lib/api/search";
import { getHome, getPage, getTree, type TreeEntry } from "@/lib/api/docs";
import CommandPalette from "@/components/editor/command-palette";
import RichEditor from "@/components/editor/rich-editor";
import { FileRowActions, NewPageForm } from "@/components/editor/file-actions";

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
  onFileDeleted: (path: string) => void;
}

function DirNode({ projectId, dir, depth, expandedDirs, onToggle, onOpen, onFileDeleted }: DirNodeProps) {
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
          <div
            className="group flex items-center gap-1 rounded-sm pr-1 hover:bg-[var(--color-surface-accent)]"
            style={{ paddingLeft: `${8 + depth * 14}px` }}
          >
            <button
              type="button"
              onClick={() => (entry.type === "tree" ? onToggle(entry) : onOpen(entry))}
              className="flex min-w-0 flex-1 items-center gap-2 py-1.5 text-left text-sm text-[var(--color-ink-2)] hover:text-[var(--color-ink)]"
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
            {entry.type === "blob" && (
              <FileRowActions
                projectId={projectId}
                path={entry.path}
                onDeleted={() => onFileDeleted(entry.path)}
              />
            )}
          </div>
          {entry.type === "tree" && (
            <DirNode
              projectId={projectId}
              dir={entry.path}
              depth={depth + 1}
              expandedDirs={expandedDirs}
              onToggle={onToggle}
              onOpen={onOpen}
              onFileDeleted={onFileDeleted}
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
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<Array<{ path: string; snippet: string }> | null>(null);
  const [searching, setSearching] = useState(false);

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

  const historyQuery = useQuery({
    queryKey: ["history", id, filePath],
    queryFn: () => fileHistory(id, filePath),
    enabled: showHistory && !showHome,
  });

  const toggleDir = (entry: TreeEntry) => {
    setExpandedDirs((prev) => {
      const next = new Set(prev);
      if (next.has(entry.path)) next.delete(entry.path);
      else next.add(entry.path);
      return next;
    });
  };

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
    setDirty(false);
    setEditing(true);
  };

  // Cmd/Ctrl+S saves; beforeunload guards against losing unsaved edits.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        if (editing && !saving) void saveEdit();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [editing, saving, draft, id, filePath]);

  useEffect(() => {
    if (!dirty) return;
    const onBeforeUnload = (e: BeforeUnloadEvent) => {
      e.preventDefault();
      e.returnValue = "";
    };
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => window.removeEventListener("beforeunload", onBeforeUnload);
  }, [dirty]);

  const cancelEdit = () => {
    if (dirty && !window.confirm("有未保存的修改，确定放弃？")) return;
    setEditing(false);
    setDirty(false);
  };

  const saveEdit = async () => {
    setSaving(true);
    try {
      const rev = await getRevision(id);
      await submitChangeset(id, {
        base_revision: rev.revision,
        message: "", // 后端生成默认：时间 + 操作者 修改 <path>
        changes: [{ op: "update", path: filePath, content: draft }],
      });
      toast.success("已保存");
      setEditing(false);
      await queryClient.invalidateQueries({ queryKey: ["docs"] });
      await queryClient.invalidateQueries({ queryKey: ["tree"] });
      setDirty(false);
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
        <div className="border-b border-[var(--color-rule)] p-2">
          <NewPageForm projectId={id} />
        </div>
        <div className="flex-1 overflow-y-auto p-2">
          <DirNode
            projectId={id}
            dir=""
            depth={0}
            expandedDirs={expandedDirs}
            onToggle={toggleDir}
            onOpen={openEntry}
            onFileDeleted={(p) => {
              if (filePath === p) navigate(`/projects/${id}/docs`);
            }}
          />
        </div>
      </aside>

      <CommandPalette projectId={id} />
      <div className="flex w-full flex-col">
        <header className="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--color-rule)] px-6 py-3">
          <Breadcrumbs projectId={id} filePath={filePath} />
          <form
            className="flex items-center gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              void runSearch();
            }}
          >
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
            <Button type="submit" variant="outline" size="sm" disabled={searching}>
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
                <Button
                  variant="ghost"
                  size="sm"
                  className="gap-2 text-[var(--color-ink-3)]"
                  onClick={() => setShowHistory((v) => !v)}
                >
                  <History className="size-3.5" />
                  历史
                </Button>
                <span className="mono-label text-[var(--color-ink-3)]">
                  {filePath}
                </span>
              </div>
            )}
            {showHistory && !editing && (
              <div className="hairline-panel mt-4 px-5">
                <p className="mono-label py-3 text-[var(--color-ink-3)]">
                  history · {filePath}
                </p>
                {historyQuery.isLoading && (
                  <p className="mono-label pb-3 text-[var(--color-ink-3)]">loading…</p>
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
            {editing && (
              <div className="space-y-3">
                <div className="flex items-center justify-between">
                  <p className="mono-label text-[var(--color-ink-3)]">
                    editing · {filePath}
                  </p>
                  <Button variant="ghost" size="sm" onClick={cancelEdit}>
                    <RefreshCw className="mr-1.5 size-3.5" />
                    取消
                  </Button>
                </div>
                {rawQuery.isLoading ? (
                  <p className="mono-label text-[var(--color-ink-3)]">loading…</p>
                ) : (
                  <div className="code-card p-1">
                    <RichEditor
                      initialMarkdown={draft}
                      onChange={(md) => {
                        setDraft(md);
                        setDirty(true);
                      }}
                    />
                  </div>
                )}
                <div className="flex justify-end gap-2">
                  <Button variant="outline" size="sm" onClick={cancelEdit}>
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
