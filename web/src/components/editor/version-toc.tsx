import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ArrowLeft, CornerDownRight, History } from "lucide-react";
import { fileHistory, type CommitSummary } from "@/lib/api/history";
import { getPage, type PageResponse } from "@/lib/api/docs";

/** TOC entry derived from rendered h1-h3 headings. */
export interface TocEntry {
  id: string;
  text: string;
  level: number;
}

export function extractToc(root: HTMLElement): TocEntry[] {
  const out: TocEntry[] = [];
  root.querySelectorAll("h1, h2, h3").forEach((el, i) => {
    const level = Number(el.tagName[1]);
    const text = (el.textContent ?? "").trim();
    if (!text) return;
    const id = `toc-${i}`;
    el.id = id;
    out.push({ id, text, level });
  });
  return out;
}

/** Sidebar TOC with click-to-scroll and active-heading highlight. */
export function TocPanel({ entries }: { entries: TocEntry[] }) {
  const [active, setActive] = useState<string | null>(null);

  useEffect(() => {
    if (entries.length === 0) return;
    const onScroll = () => {
      let current: string | null = null;
      for (const e of entries) {
        const el = document.getElementById(e.id);
        if (el && el.getBoundingClientRect().top <= 90) current = e.id;
      }
      setActive(current);
    };
    window.addEventListener("scroll", onScroll, { passive: true });
    onScroll();
    return () => window.removeEventListener("scroll", onScroll);
  }, [entries]);

  if (entries.length === 0) return null;
  return (
    <nav aria-label="目录" className="space-y-0.5 p-2">
      <p className="mono-label px-2 pb-1 text-[var(--color-ink-3)]">toc</p>
      {entries.map((e) => (
        <button
          key={e.id}
          type="button"
          onClick={() => {
            const el = document.getElementById(e.id);
            el?.scrollIntoView({ behavior: "smooth", block: "start" });
          }}
          className={`block w-full truncate rounded-sm px-2 py-1 text-left text-xs hover:bg-[var(--color-surface-accent)] ${
            active === e.id
              ? "text-[var(--color-accent)]"
              : "text-[var(--color-ink-2)]"
          }`}
          style={{ paddingLeft: `${8 + (e.level - 1) * 10}px` }}
          title={e.text}
        >
          {e.text}
        </button>
      ))}
    </nav>
  );
}

/** Version history list (shown under the TOC); clicking loads that revision. */
export function VersionPanel({
  projectId,
  filePath,
  currentVersion,
  onSelect,
}: {
  projectId: string;
  filePath: string;
  currentVersion: string | null; // null = latest
  onSelect: (sha: string | null) => void;
}) {
  const { data } = useQuery({
    queryKey: ["history", projectId, filePath],
    queryFn: () => fileHistory(projectId, filePath),
    enabled: filePath.length > 0,
  });
  if (!data || data.commits.length === 0) return null;
  return (
    <div className="space-y-0.5 p-2">
      <p className="mono-label px-2 pb-1 text-[var(--color-ink-3)]">
        versions · {data.commits.length}
      </p>
      <button
        type="button"
        onClick={() => onSelect(null)}
        className={`flex w-full items-center gap-1.5 rounded-sm px-2 py-1 text-left text-xs hover:bg-[var(--color-surface-accent)] ${
          currentVersion === null
            ? "text-[var(--color-accent)]"
            : "text-[var(--color-ink-2)]"
        }`}
      >
        <CornerDownRight className="size-3 shrink-0" />
        <span className="truncate">最新版本</span>
      </button>
      {data.commits.map((c: CommitSummary) => (
        <button
          key={c.sha}
          type="button"
          onClick={() => onSelect(c.sha)}
          className={`flex w-full items-center gap-1.5 rounded-sm px-2 py-1 text-left text-xs hover:bg-[var(--color-surface-accent)] ${
            currentVersion === c.sha
              ? "text-[var(--color-accent)]"
              : "text-[var(--color-ink-2)]"
          }`}
          title={c.message}
        >
          <History className="size-3 shrink-0" />
          <span className="truncate">{c.sha.slice(0, 7)} · {c.message}</span>
        </button>
      ))}
    </div>
  );
}

/** Loads a page (optionally at a historical revision) and extracts its TOC. */
export function useVersionedPage(projectId: string, filePath: string, atSha: string | null) {
  const query = useQuery({
    queryKey: ["docs", "page", projectId, filePath, atSha ?? "latest"],
    queryFn: (): Promise<PageResponse> =>
      getPage(projectId, filePath, "html", atSha ?? undefined),
    enabled: filePath.length > 0,
  });
  return query;
}

export { ArrowLeft };
