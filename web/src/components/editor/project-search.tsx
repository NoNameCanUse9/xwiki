import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { searchProject } from "@/lib/api/search";

interface Props {
  projectId: string;
}

/**
 * 文档视图顶栏的「搜索」按钮：打开项目级全文搜索 overlay。
 * 实时查询当前项目全部文档内容；点击结果跳转，Esc / 点击遮罩关闭。
 */
export default function ProjectSearch({ projectId }: Props) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<
    Array<{ path: string; snippet: string }>
  >([]);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const navigate = useNavigate();

  // 打开时自动聚焦。
  useEffect(() => {
    if (open) inputRef.current?.focus();
  }, [open]);

  // 防抖实时搜索。
  useEffect(() => {
    if (!open) return;
    const q = query.trim();
    if (!q) {
      setResults([]);
      return;
    }
    setBusy(true);
    const t = setTimeout(() => {
      searchProject(projectId, q, 12)
        .then((res) => setResults(res.results))
        .catch(() => setResults([]))
        .finally(() => setBusy(false));
    }, 200);
    return () => clearTimeout(t);
  }, [open, query, projectId]);

  const close = () => {
    setOpen(false);
    setQuery("");
    setResults([]);
  };

  const openPath = (path: string) => {
    close();
    navigate(`/projects/${projectId}/docs/${path}`);
  };

  return (
    <>
      <Button
        variant="ghost"
        size="sm"
        className="gap-2 text-[var(--color-ink-3)]"
        onClick={() => setOpen(true)}
      >
        <Search className="size-3.5" />
        搜索
      </Button>
      {open && (
        <div
          className="fixed inset-0 z-50 bg-[oklch(14%_0.012_258/0.5)] backdrop-blur-[2px]"
          onMouseDown={close}
        >
          <div
            className="hairline-panel mx-auto mt-24 w-full max-w-xl overflow-hidden shadow-lg"
            onMouseDown={(e) => e.stopPropagation()}
            role="dialog"
            aria-modal="true"
            aria-label="全文搜索"
          >
            <div className="flex items-center gap-2 border-b border-[var(--color-rule)] px-4 py-3">
              <Search className="size-4 shrink-0 text-[var(--color-ink-3)]" />
              <input
                ref={inputRef}
                aria-label="搜索文档"
                placeholder="搜索项目内全部文档…"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Escape") close();
                  if (e.key === "Enter" && results.length > 0) openPath(results[0].path);
                }}
                className="w-full bg-transparent font-mono text-sm text-[var(--color-ink)] placeholder:text-[var(--color-ink-3)] focus:outline-none"
              />
              <span className="mono-label shrink-0 text-[var(--color-ink-3)]">esc</span>
            </div>
            <div className="max-h-80 overflow-y-auto p-1">
              {busy && (
                <p className="mono-label px-3 py-2 text-[var(--color-ink-3)]">searching…</p>
              )}
              {!busy && query.trim() !== "" && results.length === 0 && (
                <p className="mono-label px-3 py-2 text-[var(--color-ink-3)]">no results</p>
              )}
              {results.map((r) => (
                <button
                  key={r.path}
                  type="button"
                  onClick={() => openPath(r.path)}
                  className="flex w-full items-baseline justify-between gap-3 rounded-sm px-3 py-2 text-left hover:bg-[var(--color-surface-accent)]"
                >
                  <span className="truncate font-mono text-xs text-[var(--color-accent)]">
                    {r.path}
                  </span>
                  <span className="truncate text-sm text-[var(--color-ink-2)]">{r.snippet}</span>
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
    </>
  );
}
