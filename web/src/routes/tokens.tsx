import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import {
  ArrowLeft,
  Check,
  ChevronDown,
  Copy,
  KeyRound,
  ShieldX,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { listProjects } from "@/lib/api/projects";
import { createToken, listTokens, revokeToken } from "@/lib/api/tokens";
import type { Project } from "@/lib/api/types";

interface ProjectPickerProps {
  projects: Project[];
  selectedIds: string[];
  onChange: (ids: string[]) => void;
  disabled?: boolean;
}

function ProjectPicker({
  projects,
  selectedIds,
  onChange,
  disabled = false,
}: ProjectPickerProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const selectedProjects = projects.filter((project) =>
    selectedIds.includes(project.id),
  );

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  const toggleProject = (id: string) => {
    onChange(
      selectedIds.includes(id)
        ? selectedIds.filter((selectedId) => selectedId !== id)
        : [...selectedIds, id],
    );
  };

  const buttonLabel =
    selectedProjects.length === 0
      ? "选择项目"
      : selectedProjects.length === 1
        ? selectedProjects[0].name
        : `已选择 ${selectedProjects.length} 个项目`;

  return (
    <div ref={rootRef} className="relative">
      <button
        id="tok-projects"
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => setOpen((value) => !value)}
        className="flex h-10 w-full items-center justify-between gap-3 rounded-[var(--radius)] border border-[var(--color-rule)] bg-[var(--color-paper)] px-3 text-left text-sm text-[var(--color-ink)] transition-colors hover:border-[var(--color-rule-2)] disabled:cursor-not-allowed disabled:opacity-50"
      >
        <span className={selectedProjects.length === 0 ? "text-[var(--color-ink-3)]" : ""}>
          {buttonLabel}
        </span>
        <ChevronDown
          className={`size-4 shrink-0 text-[var(--color-ink-3)] transition-transform ${open ? "rotate-180" : ""}`}
        />
      </button>
      {open && (
        <div
          role="listbox"
          aria-label="项目列表"
          aria-multiselectable="true"
          className="absolute inset-x-0 top-[calc(100%+0.5rem)] z-20 max-h-64 overflow-y-auto rounded-[var(--radius)] border border-[var(--color-rule-2)] bg-[var(--color-paper-2)] p-1 shadow-[var(--shadow-lift)]"
        >
          {projects.length === 0 ? (
            <p className="px-3 py-4 text-sm text-[var(--color-ink-3)]">
              暂无可授权项目
            </p>
          ) : (
            projects.map((project) => {
              const selected = selectedIds.includes(project.id);
              return (
                <button
                  key={project.id}
                  type="button"
                  role="option"
                  aria-selected={selected}
                  onClick={() => toggleProject(project.id)}
                  className="flex w-full items-center gap-3 rounded-[calc(var(--radius)-2px)] px-3 py-2 text-left transition-colors hover:bg-[var(--color-surface-accent)]"
                >
                  <span
                    className={`grid size-4 shrink-0 place-items-center rounded-sm border ${selected ? "border-[var(--color-accent)] bg-[var(--color-accent)] text-[var(--color-accent-ink)]" : "border-[var(--color-rule-2)]"}`}
                  >
                    {selected && <Check className="size-3" />}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm text-[var(--color-ink)]">
                      {project.name}
                    </span>
                    <span className="mono-label block truncate text-[var(--color-ink-3)]">
                      {project.id}
                      {project.archived ? " · archived" : ""}
                    </span>
                  </span>
                </button>
              );
            })
          )}
        </div>
      )}
    </div>
  );
}

export default function TokensPage() {
  const queryClient = useQueryClient();
  const { data, isLoading } = useQuery({
    queryKey: ["tokens"],
    queryFn: listTokens,
  });
  const { data: projectsData, isLoading: projectsLoading } = useQuery({
    queryKey: ["projects"],
    queryFn: listProjects,
  });
  const projects = projectsData?.projects ?? [];
  const projectNames = new Map(projects.map((project) => [project.id, project.name]));
  const [name, setName] = useState("");
  const [projectIds, setProjectIds] = useState<string[]>([]);
  const [createdSecret, setCreatedSecret] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [revokeBusy, setRevokeBusy] = useState(false);
  const [revokeTarget, setRevokeTarget] = useState<{
    id: string;
    name: string;
  } | null>(null);

  const onCreate = async () => {
    const tokenName = name.trim();
    if (!tokenName || projectIds.length === 0) {
      toast.error("名称与可访问项目必填");
      return;
    }
    setBusy(true);
    try {
      const res = await createToken({
        name: tokenName,
        scope: "write",
        project_ids: projectIds,
      });
      setCreatedSecret(res.secret);
      setName("");
      setProjectIds([]);
      await queryClient.invalidateQueries({ queryKey: ["tokens"] });
      toast.success("Token 已创建，请立即复制明文");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "创建失败");
    } finally {
      setBusy(false);
    }
  };

  const confirmRevoke = async () => {
    if (!revokeTarget) return;
    setRevokeBusy(true);
    try {
      await revokeToken(revokeTarget.id);
      setRevokeTarget(null);
      toast.success("已撤销");
      await queryClient.invalidateQueries({ queryKey: ["tokens"] });
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "撤销失败");
    } finally {
      setRevokeBusy(false);
    }
  };

  const selectedProjectNames = projects
    .filter((project) => projectIds.includes(project.id))
    .map((project) => project.name);

  return (
    <div className="flex min-h-screen flex-col">
      <header className="border-b border-[var(--color-rule)] px-5 py-4 sm:px-8">
        <div className="mx-auto flex w-full max-w-5xl items-center justify-between gap-4">
          <Link
            to="/"
            className="mono-label flex items-center gap-2 text-[var(--color-ink-3)] transition-colors hover:text-[var(--color-accent)]"
          >
            <ArrowLeft className="size-3.5" />
            workspace
          </Link>
          <span className="mono-label text-right text-[var(--color-ink-3)]">
            settings · credentials
          </span>
        </div>
      </header>

      <main className="flex-1 px-5 py-8 sm:px-8 sm:py-12">
        <div className="mx-auto w-full max-w-5xl space-y-8">
          <div className="flex flex-col justify-between gap-5 border-b border-[var(--color-rule)] pb-8 sm:flex-row sm:items-end">
            <div className="space-y-2">
              <p className="mono-label text-[var(--color-accent)]">settings / agent access</p>
              <h1 className="font-display text-3xl font-semibold leading-tight text-[var(--color-ink)] sm:text-4xl">
                Agent Token
              </h1>
              <p className="max-w-[55ch] text-[var(--color-ink-2)]">
                创建只访问指定项目的凭证，交给 AI Agent 使用。
              </p>
            </div>
            <div className="mono-label flex items-center gap-2 text-[var(--color-ink-3)]">
              <span className="size-1.5 rounded-full bg-[var(--color-success)]" />
              project-scoped
            </div>
          </div>

          <div className="grid gap-6 lg:grid-cols-[minmax(0,1.15fr)_minmax(16rem,0.85fr)]">
            <section className="hairline-panel p-5 sm:p-7">
              <div className="mb-6 flex items-start justify-between gap-4">
                <div>
                  <p className="mono-label text-[var(--color-ink-3)]">new credential</p>
                  <h2 className="mt-2 font-display text-xl font-semibold text-[var(--color-ink)]">
                    创建 Token
                  </h2>
                </div>
                <KeyRound className="size-5 text-[var(--color-accent)]" />
              </div>

              <div className="space-y-5">
                <div className="space-y-2">
                  <Label htmlFor="tok-name">名称</Label>
                  <Input
                    id="tok-name"
                    placeholder="例如：ci-bot"
                    value={name}
                    onChange={(event) => setName(event.target.value)}
                    disabled={busy}
                  />
                </div>
                <div className="space-y-2">
                  <div className="flex items-baseline justify-between gap-3">
                    <Label htmlFor="tok-projects">可访问项目</Label>
                    <span className="mono-label text-[var(--color-ink-3)]">
                      {projectIds.length} selected
                    </span>
                  </div>
                  <ProjectPicker
                    projects={projects}
                    selectedIds={projectIds}
                    onChange={setProjectIds}
                    disabled={busy || projectsLoading}
                  />
                  <p className="text-xs text-[var(--color-ink-3)]">
                    Token 只对这里选中的项目生效。
                  </p>
                </div>
              </div>

              <div className="mt-7 flex flex-col gap-3 border-t border-[var(--color-rule)] pt-5 sm:flex-row sm:items-center sm:justify-between">
                <p className="text-xs text-[var(--color-ink-3)]">
                  明文只会显示一次
                </p>
                <Button onClick={() => void onCreate()} disabled={busy} className="gap-2">
                  <KeyRound className="size-4" />
                  {busy ? "创建中…" : "创建 Token"}
                </Button>
              </div>

              {createdSecret && (
                <div className="code-card mt-5 space-y-3 p-4">
                  <div className="flex items-center justify-between gap-3">
                    <p className="mono-label text-white/50">secret · copy now</p>
                    <span className="mono-label text-[var(--color-warning)]">one time</span>
                  </div>
                  <div className="flex items-start justify-between gap-3">
                    <code className="min-w-0 break-all text-sm text-[var(--color-accent)]">
                      {createdSecret}
                    </code>
                    <Button
                      variant="outline"
                      size="sm"
                      className="shrink-0 gap-1.5"
                      onClick={() => {
                        void navigator.clipboard?.writeText(createdSecret);
                        toast.success("已复制");
                      }}
                    >
                      <Copy className="size-3.5" />
                      复制
                    </Button>
                  </div>
                </div>
              )}
            </section>

            <aside className="rounded-[var(--radius)] border border-[var(--color-rule)] bg-[var(--color-paper-2)] p-5 sm:p-7">
              <p className="mono-label text-[var(--color-ink-3)]">access model</p>
              <h2 className="mt-2 font-display text-xl font-semibold text-[var(--color-ink)]">
                按项目授权
              </h2>
              <p className="mt-3 text-sm leading-6 text-[var(--color-ink-2)]">
                每个 Token 都绑定到明确的项目，不会获得工作区内其他项目的访问权限。
              </p>
              <div className="mt-6 border-y border-[var(--color-rule)] py-4">
                <p className="mono-label text-[var(--color-ink-3)]">当前选择</p>
                {selectedProjectNames.length > 0 ? (
                  <div className="mt-3 flex flex-wrap gap-2">
                    {selectedProjectNames.map((projectName) => (
                      <span
                        key={projectName}
                        className="rounded-full border border-[var(--color-rule-2)] px-2.5 py-1 text-xs text-[var(--color-ink-2)]"
                      >
                        {projectName}
                      </span>
                    ))}
                  </div>
                ) : (
                  <p className="mt-3 text-sm text-[var(--color-ink-3)]">尚未选择项目</p>
                )}
              </div>
              <ul className="mt-5 space-y-3 text-sm text-[var(--color-ink-2)]">
                <li className="flex gap-2.5">
                  <span className="mt-2 size-1.5 shrink-0 rounded-full bg-[var(--color-accent)]" />
                  项目权限单独隔离
                </li>
                <li className="flex gap-2.5">
                  <span className="mt-2 size-1.5 shrink-0 rounded-full bg-[var(--color-accent)]" />
                  可随时撤销凭证
                </li>
              </ul>
            </aside>
          </div>

          <section className="space-y-3">
            <div className="flex items-end justify-between gap-4">
              <div>
                <p className="mono-label text-[var(--color-ink-3)]">issued tokens</p>
                <h2 className="mt-2 font-display text-xl font-semibold text-[var(--color-ink)]">
                  已创建的 Token
                </h2>
              </div>
              <span className="mono-label rounded-full border border-[var(--color-rule)] px-2.5 py-1 text-[var(--color-ink-3)]">
                {(data?.tokens ?? []).length} total
              </span>
            </div>
            {isLoading && <p className="mono-label text-[var(--color-ink-3)]">loading…</p>}
            <div className="hairline-panel overflow-hidden">
              {(data?.tokens ?? []).length === 0 && !isLoading && (
                <p className="px-5 py-10 text-center text-sm text-[var(--color-ink-2)]">
                  还没有 Token
                </p>
              )}
              {(data?.tokens ?? []).map((token) => {
                const tokenProjects = token.project_ids.map(
                  (projectId) => projectNames.get(projectId) ?? projectId,
                );
                return (
                  <div
                    key={token.id}
                    className="flex flex-col gap-4 border-b border-[var(--color-rule)] px-5 py-4 last:border-b-0 sm:flex-row sm:items-center sm:justify-between"
                  >
                    <div className="min-w-0">
                      <div className="flex items-center gap-2">
                        <p className="truncate text-sm font-medium text-[var(--color-ink)]">
                          {token.name}
                        </p>
                        {token.revoked_at && (
                          <span className="mono-label shrink-0 text-[var(--color-ink-3)]">
                            revoked
                          </span>
                        )}
                      </div>
                      <p className="mt-1 truncate text-sm text-[var(--color-ink-2)]">
                        {tokenProjects.join("、") || "未找到项目"}
                      </p>
                      <p className="mono-label mt-1 text-[var(--color-ink-3)]">
                        {token.project_ids.length} project{token.project_ids.length === 1 ? "" : "s"}
                      </p>
                    </div>
                    {!token.revoked_at && (
                      <Button
                        variant="ghost"
                        size="sm"
                        className="shrink-0 self-start gap-1.5 text-[var(--color-ink-3)] sm:self-auto"
                        onClick={() => setRevokeTarget({ id: token.id, name: token.name })}
                      >
                        <ShieldX className="size-3.5" />
                        撤销
                      </Button>
                    )}
                  </div>
                );
              })}
            </div>
          </section>
        </div>
      </main>

      <footer className="border-t border-[var(--color-rule)] px-5 py-4 sm:px-8">
        <div className="mx-auto w-full max-w-5xl">
          <p className="mono-label text-[var(--color-ink-3)]">
            agentdocs · phase 06 · credentials
          </p>
        </div>
      </footer>

      <Dialog
        open={revokeTarget !== null}
        onOpenChange={(open) => {
          if (!open && !revokeBusy) setRevokeTarget(null);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>撤销 Token</DialogTitle>
            <DialogDescription>
              确认撤销「{revokeTarget?.name}」？撤销会立即生效，之后无法恢复。
            </DialogDescription>
          </DialogHeader>
          <div className="flex justify-end gap-2 pt-2">
            <Button
              type="button"
              variant="outline"
              onClick={() => setRevokeTarget(null)}
              disabled={revokeBusy}
            >
              取消
            </Button>
            <Button
              type="button"
              variant="destructive"
              onClick={() => void confirmRevoke()}
              disabled={revokeBusy}
            >
              {revokeBusy ? "撤销中…" : "确认撤销"}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
