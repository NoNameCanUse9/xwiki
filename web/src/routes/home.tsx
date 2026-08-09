import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import {
	Archive,
	BookOpenText,
	FileText,
	KeyRound,
	LogOut,
	FolderGit2,
	RotateCcw,
	ScrollText,
	Users,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import ThemeToggle from "@/components/theme-toggle";
import ProjectCreateDialog from "@/components/project-create-dialog";
import ImportProjectDialog from "@/components/import-project-dialog";
import {
	archiveProject,
	listProjects,
	unarchiveProject,
} from "@/lib/api/projects";
import type { Project } from "@/lib/api/types";
import { useAuthStore } from "@/stores/auth";

function formatDate(iso: string): string {
	return new Date(iso).toLocaleDateString("zh-CN", {
		year: "numeric",
		month: "2-digit",
		day: "2-digit",
	});
}

function ProjectCard({ project }: { project: Project }) {
	const queryClient = useQueryClient();
	const [busy, setBusy] = useState(false);

	const onArchive = async (e: React.MouseEvent) => {
		e.preventDefault();
		e.stopPropagation();
		setBusy(true);
		try {
			await archiveProject(project.id);
			toast.success(`项目 ${project.name} 已归档`);
			await queryClient.invalidateQueries({ queryKey: ["projects"] });
		} catch (err) {
			toast.error(err instanceof Error ? err.message : "归档失败");
		} finally {
			setBusy(false);
		}
	};

	const onUnarchive = async (e: React.MouseEvent) => {
		e.preventDefault();
		e.stopPropagation();
		setBusy(true);
		try {
			await unarchiveProject(project.id);
			toast.success(`项目 ${project.name} 已恢复`);
			await queryClient.invalidateQueries({ queryKey: ["projects"] });
		} catch (err) {
			toast.error(err instanceof Error ? err.message : "恢复失败");
		} finally {
			setBusy(false);
		}
	};

	return (
		<Link
			to={`/projects/${project.id}/docs`}
			className="hairline-panel block flex flex-col gap-3 p-5 transition-colors hover:border-[var(--color-rule-2)] hover:bg-[var(--color-surface-accent)]"
		>
			<div className="flex items-start justify-between gap-3">
				<div className="min-w-0">
					<span className="font-display text-lg font-semibold text-[var(--color-ink)]">
						{project.name}
					</span>
					{project.archived && (
						<span className="mono-label ml-2 inline-block text-[var(--color-ink-3)]">
							archived
						</span>
					)}
					<p className="mt-1.5 line-clamp-2 text-sm text-[var(--color-ink-2)]">
						{project.description || "—"}
					</p>
				</div>
				{!project.archived ? (
					<Button
						variant="ghost"
						size="sm"
						disabled={busy}
						onClick={onArchive}
						className="shrink-0 gap-1.5 text-[var(--color-ink-3)]"
					>
						<Archive className="size-3.5" />
						归档
					</Button>
				) : (
					<Button
						variant="ghost"
						size="sm"
						disabled={busy}
						onClick={onUnarchive}
						className="shrink-0 gap-1.5 text-[var(--color-accent)]"
					>
						<RotateCcw className="size-3.5" />
						恢复
					</Button>
				)}
			</div>
			<div className="flex items-center gap-4 border-t border-[var(--color-rule)] pt-3">
				<span className="mono-label text-[var(--color-ink-3)]">
					{project.repo_dir}
				</span>
				<span className="mono-label ml-auto text-[var(--color-ink-3)]">
					{formatDate(project.created_at)}
				</span>
			</div>
		</Link>
	);
}

export default function HomePage() {
	const user = useAuthStore((s) => s.user);
	const logout = useAuthStore((s) => s.logout);
	const queryClient = useQueryClient();
	const { data, isLoading, isError } = useQuery({
		queryKey: ["projects"],
		queryFn: listProjects,
	});
	const displayName = user?.display_name || user?.username || "";

	const active = (data?.projects ?? []).filter((p) => !p.archived);
	const archived = (data?.projects ?? []).filter((p) => p.archived);

	return (
		<div className="flex min-h-screen">
			{/* Side rail */}
			<aside className="fixed inset-y-0 left-0 z-30 hidden w-56 shrink-0 flex-col border-r border-[var(--color-rule)] bg-[var(--color-paper-2)] sm:flex">
				<div className="border-b border-[var(--color-rule)] px-5 py-4">
					<p className="font-display text-lg font-semibold tracking-tight text-[var(--color-ink)]">
						AgentDocs
					</p>
					<p className="mono-label mt-1 text-[var(--color-ink-3)]">workspace</p>
				</div>
				<nav className="flex-1 space-y-1 overflow-y-auto px-3 py-4">
					<p className="mono-label px-2 pb-2 text-[var(--color-ink-3)]">
						projects
					</p>
					{(data?.projects ?? []).length === 0 && (
						<p className="px-2 text-sm text-[var(--color-ink-3)]">暂无项目</p>
					)}
					{(data?.projects ?? []).map((p) => (
						<Link
							key={p.id}
							to={`/projects/${p.id}/docs`}
							className="flex items-center gap-2.5 rounded-sm px-2 py-1.5 text-sm text-[var(--color-ink-2)] hover:bg-[var(--color-surface-accent)] hover:text-[var(--color-ink)]"
						>
							<FolderGit2 className="size-4 shrink-0 text-[var(--color-accent)]" />
							<span className="truncate">{p.name}</span>
						</Link>
					))}
				</nav>
				<div className="space-y-3 border-t border-[var(--color-rule)] px-4 py-4">
					<div className="flex items-center justify-between">
						<div className="min-w-0">
							<p className="truncate text-sm font-medium text-[var(--color-ink)]">
								{displayName}
							</p>
							<p className="mono-label text-[var(--color-ink-3)]">
								{user?.is_admin ? "admin" : "member"}
							</p>
						</div>
						<ThemeToggle />
					</div>
					<Link
						to="/api-docs"
						className="flex items-center gap-2 rounded-sm px-3 py-2 text-sm text-[var(--color-ink-2)] hover:bg-[var(--color-surface-accent)] hover:text-[var(--color-ink)]"
					>
						<BookOpenText className="size-4" />
						API 文档
					</Link>
					<Link
						to="/settings/tokens"
						className="flex items-center gap-2 rounded-sm px-3 py-2 text-sm text-[var(--color-ink-2)] hover:bg-[var(--color-surface-accent)] hover:text-[var(--color-ink)]"
					>
						<KeyRound className="size-4" />
						Agent Token
					</Link>
					<Link
						to="/settings/users"
						className="flex items-center gap-2 rounded-sm px-3 py-2 text-sm text-[var(--color-ink-2)] hover:bg-[var(--color-surface-accent)] hover:text-[var(--color-ink)]"
					>
						<Users className="size-4" />
						用户管理
					</Link>
					<Link
						to="/settings/audit"
						className="flex items-center gap-2 rounded-sm px-3 py-2 text-sm text-[var(--color-ink-2)] hover:bg-[var(--color-surface-accent)] hover:text-[var(--color-ink)]"
					>
						<ScrollText className="size-4" />
						审计日志
					</Link>
					<Button
						variant="outline"
						className="w-full justify-start gap-2"
						onClick={() => void logout()}
					>
						<LogOut className="size-4" />
						退出登录
					</Button>
				</div>
			</aside>

			{/* Main column */}
			<div className="flex w-full flex-col">
				<header className="flex items-center justify-between border-b border-[var(--color-rule)] px-4 py-3 sm:hidden">
					<p className="font-display text-base font-semibold text-[var(--color-ink)]">
						AgentDocs
					</p>
					<div className="flex items-center gap-2">
						<ThemeToggle />
						<Button variant="outline" size="icon" onClick={() => void logout()}>
							<LogOut className="size-4" />
						</Button>
					</div>
				</header>

				<main className="flex-1 px-6 py-10 sm:ml-56 sm:px-10">
					<div className="mx-auto w-full max-w-3xl space-y-8">
						<div className="flex flex-wrap items-end justify-between gap-4">
							<div className="space-y-2">
								<p className="mono-label text-[var(--color-accent)]">
									workspace
								</p>
								<h1 className="font-display text-3xl font-semibold leading-tight text-[var(--color-ink)] sm:text-4xl">
									项目
								</h1>
								<p className="max-w-[58ch] text-[var(--color-ink-2)]">
									每个项目对应一个独立 Git 仓库，文档即版本。
								</p>
							</div>
							<div className="flex items-center gap-2">
								<ImportProjectDialog />
								<ProjectCreateDialog
									onCreated={() => {
										void queryClient.invalidateQueries({
											queryKey: ["projects"],
										});
									}}
								/>
							</div>
						</div>

						{isLoading && (
							<p className="mono-label text-[var(--color-ink-3)]">loading…</p>
						)}
						{isError && (
							<p className="text-sm text-[var(--color-destructive)]">
								项目列表加载失败，请刷新重试。
							</p>
						)}

						{!isLoading &&
							!isError &&
							active.length === 0 &&
							archived.length === 0 && (
								<div className="hairline-panel flex flex-col items-center gap-3 px-6 py-14 text-center">
									<FileText className="size-8 text-[var(--color-ink-3)]" />
									<p className="font-display text-lg font-semibold text-[var(--color-ink)]">
										还没有项目
									</p>
									<p className="max-w-[40ch] text-sm text-[var(--color-ink-2)]">
										点击「新建项目」创建一个 Git 仓库，开始沉淀你的文档。
									</p>
								</div>
							)}

						{active.length > 0 && (
							<section className="space-y-4">
								<p className="mono-label text-[var(--color-ink-3)]">
									active · {active.length}
								</p>
								<div className="grid gap-4 lg:grid-cols-2">
									{active.map((p) => (
										<ProjectCard key={p.id} project={p} />
									))}
								</div>
							</section>
						)}

						{archived.length > 0 && (
							<section className="space-y-4">
								<p className="mono-label text-[var(--color-ink-3)]">
									archived · {archived.length}
								</p>
								<div className="grid gap-4 opacity-70 lg:grid-cols-2">
									{archived.map((p) => (
										<ProjectCard key={p.id} project={p} />
									))}
								</div>
							</section>
						)}
					</div>
				</main>

				<footer className="border-t border-[var(--color-rule)] px-6 py-4 sm:ml-56">
					<p className="mono-label text-[var(--color-ink-3)]">
						agentdocs · phase 02 · projects
					</p>
				</footer>
			</div>
		</div>
	);
}
