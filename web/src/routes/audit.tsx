import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { ArrowLeft, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { listProjects } from "@/lib/api/projects";
import { listAudit } from "@/lib/api/audit";

function formatTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString("zh-CN", { hour12: false });
}

export default function AuditPage() {
  const queryClient = useQueryClient();
  const projectsQuery = useQuery({ queryKey: ["projects"], queryFn: listProjects });
  const projects = projectsQuery.data?.projects ?? [];
  const [projectId, setProjectId] = useState("");
  // 未手动选择时默认第一个项目；select 用受控 value 落到相同结果。
  const activeProjectId = projectId || projects[0]?.id || "";

  const auditQuery = useQuery({
    queryKey: ["audit", activeProjectId],
    queryFn: () => listAudit(activeProjectId),
    enabled: activeProjectId.length > 0,
  });
  const entries = auditQuery.data?.entries ?? [];

  const refresh = () => {
    if (activeProjectId) {
      void queryClient.invalidateQueries({ queryKey: ["audit", activeProjectId] });
    }
  };

  return (
    <div className="flex min-h-screen flex-col">
      <header className="flex items-center justify-between border-b border-[var(--color-rule)] px-6 py-4">
        <Link
          to="/"
          className="mono-label flex items-center gap-2 text-[var(--color-ink-3)] hover:text-[var(--color-accent)]"
        >
          <ArrowLeft className="size-3.5" />
          workspace
        </Link>
        <span className="mono-label text-[var(--color-ink-3)]">
          settings · audit log
        </span>
      </header>

      <main className="flex-1 px-6 py-10 sm:px-10">
        <div className="mx-auto w-full max-w-3xl space-y-8">
          <div className="space-y-2">
            <p className="mono-label text-[var(--color-accent)]">settings</p>
            <h1 className="font-display text-3xl font-semibold leading-tight text-[var(--color-ink)]">
              审计日志
            </h1>
            <p className="max-w-[58ch] text-[var(--color-ink-2)]">
              按项目查看操作记录：谁在什么时间对哪个文档执行了什么操作。
            </p>
          </div>

          <section className="hairline-panel space-y-4 p-6">
            <div className="flex flex-wrap items-end justify-between gap-3">
              <div className="min-w-0 flex-1 space-y-2">
                <p className="mono-label text-[var(--color-ink-3)]">project</p>
                {projects.length === 0 ? (
                  <p className="text-sm text-[var(--color-ink-2)]">
                    暂无项目，无法查看审计日志。
                  </p>
                ) : (
                  <select
                    aria-label="选择项目"
                    value={activeProjectId}
                    onChange={(e) => setProjectId(e.target.value)}
                    className="h-9 w-full max-w-xs rounded-[var(--radius)] border border-[var(--color-rule)] bg-[var(--color-paper)] px-3 text-sm text-[var(--color-ink)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]"
                  >
                    {projects.map((p) => (
                      <option key={p.id} value={p.id}>
                        {p.name}
                      </option>
                    ))}
                  </select>
                )}
              </div>
              <Button
                variant="outline"
                size="sm"
                className="gap-1.5"
                onClick={refresh}
                disabled={!activeProjectId || auditQuery.isFetching}
              >
                <RefreshCw className="size-3.5" />
                刷新
              </Button>
            </div>
          </section>

          <section className="space-y-3">
            <p className="mono-label text-[var(--color-ink-3)]">
              entries · {entries.length}
            </p>
            {auditQuery.isLoading && (
              <p className="mono-label text-[var(--color-ink-3)]">loading…</p>
            )}
            {auditQuery.isError && (
              <p className="text-sm text-[var(--color-destructive)]">
                审计日志加载失败。
              </p>
            )}
            {!auditQuery.isLoading &&
              !auditQuery.isError &&
              activeProjectId &&
              entries.length === 0 && (
                <p className="hairline-panel px-6 py-10 text-center text-sm text-[var(--color-ink-2)]">
                  暂无审计记录
                </p>
              )}
            {!auditQuery.isLoading &&
              !auditQuery.isError &&
              activeProjectId &&
              entries.length > 0 && (
                <div className="hairline-panel divide-y divide-[var(--color-rule)] px-5">
                  {entries.map((e) => (
                    <div
                      key={e.id}
                      className="flex flex-wrap items-center gap-3 py-3"
                    >
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-sm font-medium text-[var(--color-ink)]">
                          <span className="font-mono text-xs text-[var(--color-accent)]">
                            {e.action}
                          </span>
                          <span className="ml-2 text-[var(--color-ink-2)]">
                            {e.actor_type}:{e.actor_id}
                          </span>
                        </p>
                        <p className="mono-label mt-0.5 truncate text-[var(--color-ink-3)]">
                          {e.path || "—"}
                        </p>
                      </div>
                      <span className="mono-label shrink-0 text-[var(--color-ink-3)]">
                        {formatTime(e.created_at)}
                      </span>
                    </div>
                  ))}
                </div>
              )}
          </section>
        </div>
      </main>

      <footer className="border-t border-[var(--color-rule)] px-6 py-4">
        <p className="mono-label text-[var(--color-ink-3)]">
          agentdocs · audit log
        </p>
      </footer>
    </div>
  );
}
