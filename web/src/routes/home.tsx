import { FileText, LogOut } from "lucide-react";
import { Button } from "@/components/ui/button";
import ThemeToggle from "@/components/theme-toggle";
import { useAuthStore } from "@/stores/auth";

export default function HomePage() {
  const user = useAuthStore((s) => s.user);
  const logout = useAuthStore((s) => s.logout);
  const displayName = user?.display_name || user?.username || "";

  return (
    <div className="flex min-h-screen">
      {/* Side rail — N3 · the application frame */}
      <aside className="hidden w-56 shrink-0 flex-col border-r border-[var(--color-rule)] bg-[var(--color-paper-2)] sm:flex">
        <div className="border-b border-[var(--color-rule)] px-5 py-4">
          <p className="font-display text-lg font-semibold tracking-tight text-[var(--color-ink)]">
            AgentDocs
          </p>
          <p className="mono-label mt-1 text-[var(--color-ink-3)]">
            workspace
          </p>
        </div>

        <nav className="flex-1 space-y-1 px-3 py-4">
          <p className="mono-label px-2 pb-2 text-[var(--color-ink-3)]">
            projects
          </p>
          <div className="hairline-panel flex items-center gap-2.5 px-3 py-2.5">
            <FileText className="size-4 text-[var(--color-ink-3)]" />
            <span className="text-sm text-[var(--color-ink-3)]">
              项目列表将在阶段二实现
            </span>
          </div>
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

      {/* Mobile top bar — same frame, stacked */}
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

        <main className="flex-1 px-6 py-10 sm:px-10">
          <div className="mx-auto w-full max-w-3xl space-y-8">
            <div className="space-y-2">
              <p className="mono-label text-[var(--color-accent)]">
                authenticated
              </p>
              <h1 className="font-display text-3xl font-semibold leading-tight text-[var(--color-ink)] sm:text-4xl">
                你好，{displayName}
              </h1>
              <p className="max-w-[58ch] text-[var(--color-ink-2)]">
                阶段一骨架已就绪：登录、会话持久化、前端构建嵌入。
                下一阶段将引入项目与 Git 仓库。
              </p>
            </div>

            <div className="grid gap-4 sm:grid-cols-2">
              <div className="hairline-panel space-y-2 p-5">
                <p className="mono-label text-[var(--color-ink-3)]">
                  storage
                </p>
                <p className="text-sm leading-relaxed text-[var(--color-ink-2)]">
                  SQLite · WAL · goose 迁移 · users / sessions
                </p>
              </div>
              <div className="hairline-panel space-y-2 p-5">
                <p className="mono-label text-[var(--color-ink-3)]">
                  auth
                </p>
                <p className="text-sm leading-relaxed text-[var(--color-ink-2)]">
                  Argon2id · HttpOnly Cookie · 服务重启会话保持
                </p>
              </div>
              <div className="hairline-panel space-y-2 p-5 sm:col-span-2">
                <p className="mono-label text-[var(--color-ink-3)]">
                  next
                </p>
                <p className="text-sm leading-relaxed text-[var(--color-ink-2)]">
                  阶段二 · 项目与 Git 仓库：创建项目、一项目一仓库、README 初始化
                </p>
              </div>
            </div>
          </div>
        </main>

        <footer className="border-t border-[var(--color-rule)] px-6 py-4">
          <p className="mono-label text-[var(--color-ink-3)]">
            agentdocs · phase 01 · skeleton
          </p>
        </footer>
      </div>
    </div>
  );
}
