import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "react-router-dom";
import { ArrowLeft, ChevronRight, CornerDownRight, FileText, Folder, FolderOpen, History, Pencil, RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { getRevision, submitChangeset } from "@/lib/api/changesets";
import { fileHistory } from "@/lib/api/history";
import { searchProject } from "@/lib/api/search";
import { getPage, getTree, type TreeEntry } from "@/lib/api/docs";
import CommandPalette from "@/components/editor/command-palette";
import RichEditor from "@/components/editor/rich-editor";
import { FileRowActions, NewPageForm } from "@/components/editor/file-actions";
import FileMenu from "@/components/editor/file-menu";
import ImportFilesButton from "@/components/editor/import-files";
import AttachmentsPanel from "@/components/editor/attachments";
import { enhanceRenderedMarkdown } from "@/components/editor/markdown-render";
import {
  extractToc,
  TocPanel,
  VersionPanel,
  useVersionedPage,
  type TocEntry,
} from "@/components/editor/version-toc";
import { backlinks } from "@/lib/api/search";

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

function MarkdownArticle({
  html,
  onNavigate,
  onToc,
}: {
  html: string;
  onNavigate: (path: string) => void;
  onToc: (entries: TocEntry[]) => void;
}) {
  const ref = useRef<HTMLElement>(null);
  useEffect(() => {
    if (ref.current) {
      void enhanceRenderedMarkdown(ref.current).then(() => {
        onToc(extractToc(ref.current!));
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [html]);
  return (
    <article
      ref={ref}
      className="prose-agentdocs"
      dangerouslySetInnerHTML={{ __html: html }}
      onClick={(e) => {
        const anchor = (e.target as HTMLElement).closest("a");
        if (!anchor) return;
        const href = anchor.getAttribute("href") ?? "";
        if (href.startsWith("/projects/")) {
          e.preventDefault();
          onNavigate(href);
        }
      }}
    />
  );
}

function BacklinksPanel({ projectId, filePath }: { projectId: string; filePath: string }) {
  const navigate = useNavigate();
  const { data } = useQuery({
    queryKey: ["backlinks", projectId, filePath],
    queryFn: () => backlinks(projectId, filePath),
    enabled: filePath.length > 0,
  });
  const items = data?.backlinks ?? [];
  return (
    <section className="space-y-3">
      <p className="mono-label flex items-center gap-2 text-[var(--color-ink-3)]">
        <CornerDownRight className="size-3.5" />
        backlinks · {items.length}
      </p>
      {items.length === 0 ? (
        <p className="hairline-panel px-4 py-5 text-center text-sm text-[var(--color-ink-2)]">
          暂无其他页面引用本文档
        </p>
      ) : (
        <div className="hairline-panel divide-y divide-[var(--color-rule)] px-4">
          {items.map((b) => (
            <button
              key={b.source}
              type="button"
              onClick={() => navigate(`/projects/${projectId}/docs/${b.source}`)}
              className="block w-full py-2.5 text-left hover:bg-[var(--color-surface-accent)]"
            >
              <span className="font-mono text-xs text-[var(--color-accent)]">{b.source}</span>
              <span className="ml-3 text-sm text-[var(--color-ink-2)]">{b.snippet}</span>
            </button>
          ))}
        </div>
      )}
    </section>
  );
}

function FileExplorer({
  projectId,
  dirPath,
  depth,
  defaultExpanded,
  onOpen,
}: {
  projectId: string;
  dirPath: string;
  depth: number;
  defaultExpanded?: boolean;
  onOpen: (path: string) => void;
}) {
  const [expanded] = useState(defaultExpanded ?? false);
  const { data, isLoading } = useQuery({
    queryKey: ["dir", projectId, dirPath],
    queryFn: () => getTree(projectId, dirPath),
    enabled: expanded || depth === 0,
  });
  const dirs = (data?.tree ?? []).filter((e) => e.type === "tree");
  const files = (data?.tree ?? []).filter((e) => e.type === "blob" && e.path !== "_sidebar.md");
  const isRoot = depth === 0;
  const itemCount = dirs.length + files.length;

  if (isRoot) {
    return (
      <section>
        <div className="mb-1 flex items-center gap-2 px-4 py-2.5">
          <span className="mono-label text-[var(--color-ink-3)]">
            root · {itemCount} {itemCount === 1 ? "item" : "items"}
          </span>
        </div>
        {isLoading && (
          <p className="px-4 py-3 text-xs text-[var(--color-ink-3)]">loading…</p>
        )}
        {!isLoading && itemCount === 0 && (
          <p className="hairline-panel mx-4 my-6 px-4 py-8 text-center text-sm text-[var(--color-ink-2)]">
            空目录
          </p>
        )}
        {!isLoading && itemCount > 0 && (
          <div className="divide-y divide-[var(--color-rule)] border-t border-[var(--color-rule)]">
            {dirs.map((d) => (
              <ExpandableRow
                key={d.path}
                projectId={projectId}
                entry={d}
                depth={0}
                onOpen={onOpen}
              />
            ))}
            {files.map((f) => (
              <button
                key={f.path}
                type="button"
                onClick={() => onOpen(f.path)}
                className="flex w-full items-center gap-3 px-4 py-2.5 text-left text-sm hover:bg-[var(--color-surface-accent)]"
              >
                <FileText className="size-4 shrink-0 text-[var(--color-ink-3)]" />
                <span className="text-[var(--color-ink)]">{f.name}</span>
                <span className="mono-label ml-auto text-[var(--color-ink-3)]">
                  {f.path}
                </span>
              </button>
            ))}
          </div>
        )}
      </section>
    );
  }
  return null;
}

function ExpandableRow({
  projectId,
  entry,
  depth,
  onOpen,
}: {
  projectId: string;
  entry: TreeEntry;
  depth: number;
  onOpen: (path: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const { data } = useQuery({
    queryKey: ["dir", projectId, entry.path],
    queryFn: () => getTree(projectId, entry.path),
    enabled: expanded,
  });
  const children = data?.tree ?? [];
  const childDirs = children.filter((e) => e.type === "tree");
  const childFiles = children.filter((e) => e.type === "blob" && e.path !== "_sidebar.md");
  const childCount = childDirs.length + childFiles.length;
  const indent = depth * 20 + 28;
  return (
    <div>
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className="flex w-full items-center gap-3 px-4 py-2.5 text-left text-sm hover:bg-[var(--color-surface-accent)]"
      >
        <ChevronRight
          className={`size-4 shrink-0 text-[var(--color-ink-3)] transition-transform ${
            expanded ? "rotate-90" : ""
          }`}
        />
        <Folder className="size-4 shrink-0 text-[var(--color-accent)]" />
        <span className="text-[var(--color-ink)]">{entry.name}</span>
        {!expanded && childCount > 0 && (
          <span className="mono-label ml-auto text-[var(--color-ink-3)]">
            {childCount}
          </span>
        )}
        <span className="mono-label ml-auto text-[var(--color-ink-3)]">
          {entry.path}/
        </span>
      </button>
      {expanded && (
        <div>
          {childDirs.map((d) => (
            <div key={d.path} style={{ paddingLeft: `${indent}px` }}>
              <ExpandableRow
                projectId={projectId}
                entry={d}
                depth={depth + 1}
                onOpen={onOpen}
              />
            </div>
          ))}
          {childFiles.map((f) => (
            <button
              key={f.path}
              type="button"
              onClick={() => onOpen(f.path)}
              style={{ paddingLeft: `${indent}px` }}
              className="flex w-full items-center gap-3 px-4 py-2.5 text-left text-sm hover:bg-[var(--color-surface-accent)]"
            >
              <FileText className="size-4 shrink-0 text-[var(--color-ink-3)]" />
              <span className="text-[var(--color-ink)]">{f.name}</span>
              <span className="mono-label ml-auto text-[var(--color-ink-3)]">
                {f.path}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
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
    select: (res) => ({
      ...res,
      tree: res.tree.filter((e) => e.path !== "_sidebar.md"),
    }),
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
                  <Folder className="size-3.5function FileExplorer({
  projectId,
  dirPath,
  depth,
  defaultExpanded,
  onOpen,
}: {
  projectId: string;
  dirPath: string;
  depth: number;
  defaultExpanded?: boolean;
  onOpen: (path: string) => void;
}) {
  const [expanded, setExpanded] = useState(defaultExpanded ?? false);
  const { data, isLoading } = useQuery({
    queryKey: ['dir', projectId, dirPath],
    queryFn: () => getTree(projectId, dirPath),
    enabled: expanded || depth === 0,
  });
  const dirs = (data?.tree ?? []).filter((e) => e.type === 'tree');
  const files = (data?.tree ?? []).filter((e) => e.type === 'blob' && e.path !== '_sidebar.md');
  const isRoot = depth === 0;
  const itemCount = dirs.length + files.length;

  if (isRoot) {
    return (
      <section>
        <div className='mb-1 flex items-center gap-2 px-4 py-2.5'>
          <span className='mono-label text-[var(--color-ink-3)]'>
            root · {itemCount} {itemCount === 1 ? 'item' : 'items'}
          </span>
        </div>
        {isLoading && (
          <p className='px-4 py-3 text-xs text-[var(--color-ink-3)]'>loading…</p>
        )}
        {!isLoading && itemCount === 0 && (
          <p className='hairline-panel mx-4 my-6 px-4 py-8 text-center text-sm text-[var(--color-ink-2)]'>
            空目录
          </p>
        )}
        {!isLoading && itemCount > 0 && (
          <div className='divide-y divide-[var(--color-rule)] border-t border-[var(--color-rule)]'>
            {dirs.map((d) => (
              <ExpandableRow
                key={d.path}
                projectId={projectId}
                entry={d}
                depth={0}
                onOpen={onOpen}
              />
            ))}
            {files.map((f) => (
              <button
                key={f.path}
                type='button'
                onClick={() => onOpen(f.path)}
                className='flex w-full items-center gap-3 px-4 py-2.5 text-left text-sm hover:bg-[var(--color-surface-accent)]'
              >
                <FileText className='size-4 shrink-0 text-[var(--color-ink-3)]' />
                <span className='text-[var(--color-ink)]'>{f.name}</span>
                <span className='mono-label ml-auto text-[var(--color-ink-3)]'>
                  {f.path}
                </span>
              </button>
            ))}
          </div>
        )}
      </section>
    );
  }
  // Non-root: render inline expandable (used in sidebar tree via DirNode).
  return null;
}

function ExpandableRow({
  projectId,
  entry,
  depth,
  onOpen,
}: {
  projectId: string;
  entry: TreeEntry;
  depth: number;
  onOpen: (path: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const { data } = useQuery({
    queryKey: ['dir', projectId, entry.path],
    queryFn: () => getTree(projectId, entry.path),
    enabled: expanded,
  });
  const children = data?.tree ?? [];
  const childDirs = children.filter((e) => e.type === 'tree');
  const childFiles = children.filter((e) => e.type === 'blob' && e.path !== '_sidebar.md');
  const childCount = childDirs.length + childFiles.length;
  return (
    <div>
      <button
        type='button'
        onClick={() => setExpanded((v) => !v)}
        className='flex w-full items-center gap-3 px-4 py-2.5 text-left text-sm hover:bg-[var(--color-surface-accent)]'
      >
        <ChevronRight
          className={}
        />
        <Folder className='size-4 shrink-0 text-[var(--color-accent)]' />
        <span className='text-[var(--color-ink)]'>{entry.name}</span>
        {!expanded && childCount > 0 && (
          <span className='mono-label ml-auto text-[var(--color-ink-3)]'>
            {childCount}
          </span>
        )}
        <span className='mono-label ml-auto text-[var(--color-ink-3)]'>
          {entry.path}/
        </span>
      </button>
      {expanded && (
        <div>
          {childDirs.map((d) => (
            <div key={d.path} style={{ paddingLeft:  }}>
              <ExpandableRow
                projectId={projectId}
                entry={d}
                depth={depth + 1}
                onOpen={onOpen}
              />
            </div>
          ))}
          {childFiles.map((f) => (
            <button
              key={f.path}
              type='button'
              onClick={() => onOpen(f.path)}
              style={{ paddingLeft:  }}
              className='flex w-full items-center gap-3 px-4 py-2.5 text-left text-sm hover:bg-[var(--color-surface-accent)]'
            >
              <FileText className='size-4 shrink-0 text-[var(--color-ink-3)]' />
              <span className='text-[var(--color-ink)]'>{f.name}</span>
              <span className='mono-label ml-auto text-[var(--color-ink-3)]'>
                {f.path}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

 shrink-0 text-[var(--color-accent)]" />
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
  const [showAttachments, setShowAttachments] = useState(false);
  const [showBacklinks, setShowBacklinks] = useState(false);
  const [atSha, setAtSha] = useState<string | null>(null);
  const [tocEntries, setTocEntries] = useState<TocEntry[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<Array<{ path: string; snippet: string }> | null>(null);
  const [searching, setSearching] = useState(false);

  const showHome = !filePath;
  const isDirPath = filePath.length > 0 && filePath.endsWith("/");
  const dirPath = isDirPath ? filePath.slice(0, -1) : "";

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

  // Custom sidebar menu from _sidebar.md at the repo root (OtterWiki-style).
  const sidebarQuery = useQuery({
    queryKey: ["docs", "sidebar", id],
    queryFn: () => getPage(id, "_sidebar.md"),
    enabled: true,
  });
  const sidebarItems = useMemo(() => {
    const raw = sidebarQuery.data?.content ?? "";
    const items: Array<{ label: string; path: string }> = [];
    const re = /^[-*]\s+\[([^\]]+)\]\(([^)]+)\)/gm;
    let m: RegExpExecArray | null;
    while ((m = re.exec(raw)) !== null) {
      items.push({ label: m[1], path: m[2] });
    }
    return items;
  }, [sidebarQuery.data]);

  const pageQuery = useVersionedPage(id, filePath, atSha);

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

  const selectVersion = (sha: string | null) => {
    setAtSha(sha);
    setTocEntries([]);
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

  const content = pageQuery.data;
  const loading = pageQuery.isLoading;
  const error = pageQuery.isError;

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
            onFileDeleted={(p) => {
              if (filePath === p) navigate(`/projects/${id}/docs`);
            }}
          />
        </div>
        <div className="border-t border-[var(--color-rule)] p-2">
          <NewPageForm projectId={id} />
          {sidebarItems.length > 0 && (
            <nav className="mt-2 space-y-0.5 border-t border-[var(--color-rule)] pt-2">
              <p className="mono-label px-2 pb-1 text-[var(--color-ink-3)]">menu</p>
              {sidebarItems.map((item) => (
                <button
                  key={item.path}
                  type="button"
                  onClick={() => navigate(`/projects/${id}/docs/${item.path}`)}
                  className="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm text-[var(--color-ink-2)] hover:bg-[var(--color-surface-accent)] hover:text-[var(--color-ink)]"
                >
                  <span className="truncate">{item.label}</span>
                </button>
              ))}
            </nav>
          )}
        </div>
        {!showHome && !editing && (
          <div className="border-t border-[var(--color-rule)] p-2">
            <TocPanel entries={tocEntries} />
            <VersionPanel
              projectId={id}
              filePath={filePath}
              currentVersion={atSha}
              onSelect={selectVersion}
            />
          </div>
        )}
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
            {atSha && (
              <div className="mb-4 flex items-center justify-between gap-3 rounded-[var(--radius)] border border-[var(--color-accent)] bg-[var(--color-surface-accent)] px-4 py-2.5">
                <p className="mono-label text-[var(--color-ink-2)]">
                  viewing historical version {atSha.slice(0, 7)}
                </p>
                <Button size="sm" variant="outline" onClick={() => selectVersion(null)}>
                  返回最新版本
                </Button>
              </div>
            )}
            {content && content.format === "html" && !editing && (
              <MarkdownArticle
                html={sanitizeHtml(content.content)}
                onToc={setTocEntries}
                onNavigate={(href) => {
                  const m = href.match(/^\/projects\/[^/]+\/docs\/(.+)$/);
                  if (m) navigate(`/projects/${id}/docs/${m[1]}`);
                }}
              />
            )}
            {content && content.format === "raw" && !editing && (
              <pre className="code-card overflow-x-auto p-4">{content.content}</pre>
            )}
            {isDirPath && (
              <FileExplorer
                projectId={id}
                dirPath={dirPath}
                depth={0}
                defaultExpanded
                onOpen={(p) => navigate(`/projects/${id}/docs/${p}`)}
              />
            )}
            {showHome && !loading && !error && (
              <FileExplorer
                projectId={id}
                dirPath=""
                depth={0}
                defaultExpanded
                onOpen={(path) => navigate(`/projects/${id}/docs/${path}`)}
              />
            )}
            {!showHome && !editing && showBacklinks && (
              <div className="mt-10">
                <BacklinksPanel projectId={id} filePath={filePath} />
              </div>
            )}
            {!showHome && !editing && showAttachments && (
              <div className="mt-10">
                <AttachmentsPanel projectId={id} />
              </div>
            )}
            {!showHome && !editing && !isDirPath && (
              <div className="mt-6 flex items-center justify-between gap-3">
                <div className="flex items-center gap-3">
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
                  <ImportFilesButton projectId={id} />
                </div>
                <FileMenu
                  projectId={id}
                  filePath={filePath}
                  items={{
                    onEdit: startEdit,
                    onToggleHistory: () => setShowHistory((v) => !v),
                    onToggleAttachments: () => setShowAttachments((v) => !v),
                    onToggleBacklinks: () => setShowBacklinks((v) => !v),
                  }}
                />
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
