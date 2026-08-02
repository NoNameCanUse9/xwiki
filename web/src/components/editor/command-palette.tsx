import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Search } from "lucide-react";
import { searchProject } from "@/lib/api/search";

interface Props {
  projectId: string;
}

/**
 * Cmd/Ctrl+K command palette: fuzzy-jump to any document in the project.
 * Renders a dialog overlay with a debounced search; Enter opens the top hit.
 */
export default function CommandPalette({ projectId }: Props) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Array<{ path: string; snippet: string }>>([]);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const navigate = useNavigate();

  // Global Cmd/Ctrl+K to open.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen((v) => !v);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Debounced search while open.
  useEffect(() => {
    if (!open) return;
    const q = query.trim();
    if (!q) {
      setResults([]);
      return;
    }
    setBusy(true);
    const t = setTimeout(() => {
      searchProject(projectId, q, 8)
        .then((res) => setResults(res.results))
        .catch(() => setResults([]))
        .finally(() => setBusy(false));
    }, 200);
    return () => clearTimeout(t);
  }, [open, query, projectId]);

  // Autofocus on open.
  useEffect(() => {
    if (open) inputRef.current?.focus();
  }, [open]);

  const close = () => {
    setOpen(false);
    setQuery("");
    setResults([]);
  };

  const openPath = (path: string) => {
    close();
    navigate(`/projects/${projectId}/docs/${path}`);
  };

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 bg-[oklch(14%_0.012_258/0.5)] backdrop-blur-[2px]"
      onMouseDown={close}
    >
      <div
        className="hairline-panel mx-auto mt-24 w-full max-w-xl overflow-hidden shadow-lg"
        onMouseDown={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label="命令面板"
      >
        <div className="flex items-center gap-2 border-b border-[var(--color-rule)] px-4 py-3">
          <Search className="size-4 shrink-0 text-[var(--color-ink-3)]" />
          <input
            ref={inputRef}
            aria-label="搜索或跳转"
            placeholder="搜索或跳转…"
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
          {busy && <p className="mono-label px-3 py-2 text-[var(--color-ink-3)]">searching…</p>}
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
  );
}
