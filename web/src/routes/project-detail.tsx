import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { ArrowLeft, BookOpen, FolderGit2, History, RotateCcw } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { getProject } from "@/lib/api/projects";
import { getHome } from "@/lib/api/docs";
import {
  getCommitDiff,
  listCommits,
  revertCommit,
  type CommitSummary,
} from "@/lib/api/history";

function shortSha(sha: string): string {
  return sha.slice(0, 8);
}

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
}

function CommitRow({ projectId, commit }: { projectId: string; commit: CommitSummary }) {
  const queryClient = useQueryClient();
  const { data } = useQuery({
    queryKey: ["diff", projectId, commit.sha],
    queryFn: () => getCommitDiff(projectId, commit.sha, "numstat"),
  });

  const onRevert = async () => {
    if (!window.confirm(`确认回滚提交 ${shortSha(commit.sha)}？将创建一个新提交。`)) {
      return;
    }
    try {
      await revertCommit(projectId, commit.sha);
      toast.success("已回滚");
      await queryClient.invalidateQueries({ queryKey: ["commits"] });
      await queryClient.invalidateQueries({ queryKey: ["docs"] });
      await queryClient.invalidateQueries({ queryKey: ["tree"] });
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "回滚失败");
    }
  };

  return (
    <div className="space-y-2 border-b border-[var(--color-rule)] py-3 last:border-b-0">
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <p className="truncate font-mono text-xs text-[var(--color-accent)]">
            {shortSha(commit.sha)}
          </p>
          <p className="truncate text-sm text-[var(--color-ink)]">{commit.message}</p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <span className="mono-label text-[var(--color-ink-3)]">
            {formatDate(commit.date)}
          </span>
          <Button variant="ghost" size="sm" onClick={() => void onRevert()} className="gap-1.5 text-[var(--color-ink-3)]">
            <RotateCcw className="size-3.5" />
            回滚
          </Button>
        </div>
      </div>
      {data && data.stats.length > 0 && (
        <div className="space-y-0.5 pl-1">
          {data.stats.map((s) => (
            <p key={s.path} className="mono-label !normal-case text-[var(--color-ink-3)]">
              {s.path}{" "}
              <span className="text-[oklch(55%_0.13_145)]">+{s.added}</span>{" "}
              <span className="text-[var(--color-destructive)]">-{s.deleted}</span>
            </p>
          ))}
        </div>
      )}
    </div>
  );
}

export default function ProjectDetailPage() {
  const { id = "" } = useParams();
	const [commitSearch, setCommitSearch] = useState("");
	const [commitQuery, setCommitQuery] = useState("");
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

	const commitsQuery = useInfiniteQuery({
		queryKey: ["commits", id, commitQuery],
		queryFn: ({ pageParam }) => listCommits(id, 5, pageParam, commitQuery),
		initialPageParam: 0,
		getNextPageParam: (lastPage, pages) =>
			lastPage.has_more
				? pages.reduce((count, page) => count + page.commits.length, 0)
				: undefined,
		enabled: id.length > 0,
	});
	const commits = useMemo(
		() => commitsQuery.data?.pages.flatMap((page) => page.commits) ?? [],
		[commitsQuery.data],
	);

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
				<form
					className="flex gap-2"
					onSubmit={(event) => {
						event.preventDefault();
						setCommitQuery(commitSearch.trim());
					}}
				>
					<Input
						aria-label="搜索提交历史"
						value={commitSearch}
						onChange={(event) => setCommitSearch(event.target.value)}
						placeholder="消息、作者或 SHA"
					/>
					<Button type="submit" variant="outline">搜索提交</Button>
				</form>
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

              <section className="space-y-4">
                <div className="flex items-center justify-between">
                  <p className="mono-label text-[var(--color-ink-3)]">
                    recent commits
                  </p>
                  <History className="size-4 text-[var(--color-ink-3)]" />
                </div>
                {commitsQuery.isLoading && (
                  <p className="mono-label text-[var(--color-ink-3)]">loading…</p>
                )}
				{commitsQuery.data && (
					<div className="hairline-panel px-5">
						{commits.length === 0 && (
                      <p className="py-6 text-center text-sm text-[var(--color-ink-2)]">
                        还没有提交
                      </p>
                    )}
						{commits.map((c) => (
							<CommitRow key={c.sha} projectId={project.id} commit={c} />
						))}
						{commitsQuery.hasNextPage && (
							<div className="border-t border-[var(--color-rule)] py-3 text-center">
								<Button
									variant="ghost"
									disabled={commitsQuery.isFetchingNextPage}
									onClick={() => void commitsQuery.fetchNextPage()}
								>
									{commitsQuery.isFetchingNextPage ? "加载中…" : "加载更多"}
								</Button>
							</div>
						)}
					</div>
                )}
              </section>
            </>
          )}
        </div>
      </main>

      <footer className="border-t border-[var(--color-rule)] px-6 py-4">
        <p className="mono-label text-[var(--color-ink-3)]">
          xwiki · phase 02 · projects
        </p>
      </footer>
    </div>
  );
}
