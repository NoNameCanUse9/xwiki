import { useQuery } from "@tanstack/react-query";
import { Link, useParams } from "react-router-dom";
import { ArrowLeft, BookOpen, FolderGit2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { getProject } from "@/lib/api/projects";
import { getHome } from "@/lib/api/docs";

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
}

export default function ProjectDetailPage() {
  const { id = "" } = useParams();
  const { data, isLoading, isError } = useQuery({
    queryKey: ["projects", id],
    queryFn: () => getProject(id),
    enabled: id.length > 0,
  });
  const project = data?.project;

  const homeQuery = useQuery({
    queryKey: ["docs", "home", id],
    queryFn: () => getHome(id),
    enabled: id.length > 0,
  });
  const homeHtml = homeQuery.data?.content ?? "";

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
          project · detail
        </span>
      </header>

      <main className="flex-1 px-6 py-10 sm:px-10">
        <div className="mx-auto w-full max-w-3xl space-y-8">
          {isLoading && (
            <p className="mono-label text-[var(--color-ink-3)]">loading…</p>
          )}
          {isError && (
            <div className="hairline-panel px-6 py-10 text-center">
              <p className="font-display text-lg font-semibold text-[var(--color-ink)]">
                项目不存在或已被移除
              </p>
              <p className="mt-2 text-sm text-[var(--color-ink-2)]">
                <Link to="/" className="text-[var(--color-accent)] hover:underline">
                  返回工作台
                </Link>
              </p>
            </div>
          )}

          {project && (
            <>
              <div className="space-y-2">
                <p className="mono-label text-[var(--color-accent)]">
                  {project.archived ? "archived" : "active"}
                </p>
                <h1 className="font-display text-3xl font-semibold leading-tight text-[var(--color-ink)] sm:text-4xl">
                  {project.name}
                </h1>
                <p className="max-w-[58ch] text-[var(--color-ink-2)]">
                  {project.description || "（无描述）"}
                </p>
              </div>

              <div className="grid gap-4 sm:grid-cols-2">
                <div className="hairline-panel space-y-2 p-5">
                  <p className="mono-label text-[var(--color-ink-3)]">
                    repository
                  </p>
                  <p className="flex items-center gap-2 font-mono text-sm text-[var(--color-ink)]">
                    <FolderGit2 className="size-4 text-[var(--color-accent)]" />
                    {project.repo_dir}
                  </p>
                </div>
                <div className="hairline-panel space-y-2 p-5">
                  <p className="mono-label text-[var(--color-ink-3)]">
                    created
                  </p>
                  <p className="text-sm text-[var(--color-ink-2)]">
                    {formatDate(project.created_at)}
                  </p>
                </div>
              </div>

              <section className="space-y-4">
                <div className="flex items-center justify-between">
                  <p className="mono-label text-[var(--color-ink-3)]">
                    readme
                  </p>
                  <Link to={`/projects/${project.id}/docs`}>
                    <Button size="sm" className="gap-2">
                      <BookOpen className="size-4" />
                      阅读文档
                    </Button>
                  </Link>
                </div>
                {homeQuery.isLoading && (
                  <p className="mono-label text-[var(--color-ink-3)]">loading…</p>
                )}
                {homeHtml ? (
                  <div
                    className="prose-agentdocs hairline-panel max-h-72 overflow-y-auto p-5"
                    dangerouslySetInnerHTML={{ __html: homeHtml }}
                  />
                ) : (
                  !homeQuery.isLoading && (
                    <div className="hairline-panel px-6 py-8 text-center">
                      <p className="text-sm text-[var(--color-ink-2)]">
                        项目还没有 README，从文档树开始阅读。
                      </p>
                    </div>
                  )
                )}
              </section>
            </>
          )}
        </div>
      </main>

      <footer className="border-t border-[var(--color-rule)] px-6 py-4">
        <p className="mono-label text-[var(--color-ink-3)]">
          agentdocs · phase 02 · projects
        </p>
      </footer>
    </div>
  );
}
