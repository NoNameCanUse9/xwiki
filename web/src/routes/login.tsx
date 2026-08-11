import { useState } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import ThemeToggle from "@/components/theme-toggle";
import { useAuthStore } from "@/stores/auth";

const schema = z.object({
  username: z.string().min(1, "请输入用户名"),
  password: z.string().min(1, "请输入密码"),
});

type FormValues = z.infer<typeof schema>;

function TerminalCard() {
  return (
    <div className="code-card overflow-hidden">
      <div className="flex items-center justify-between border-b border-white/10 px-4 py-2.5">
        <div className="flex gap-1.5">
          <span className="size-2.5 rounded-full bg-white/15" />
          <span className="size-2.5 rounded-full bg-white/15" />
          <span className="size-2.5 rounded-full bg-white/15" />
        </div>
        <span className="mono-label !normal-case text-white/40">
          xwiki — session
        </span>
      </div>
      <div className="space-y-1.5 px-4 py-4">
        <p>
          <span className="tok-key">$</span>{" "}
          <span className="text-white/85">xwiki admin create -username admin</span>
        </p>
        <p className="tok-muted">› argon2id · session persisted to sqlite</p>
        <p className="tok-ok">✓ 200 OK — xwiki_session set (HttpOnly)</p>
      </div>
    </div>
  );
}

export default function LoginPage() {
  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<FormValues>({ resolver: zodResolver(schema) });
  const [error, setError] = useState<string | null>(null);
  const login = useAuthStore((s) => s.login);
  const navigate = useNavigate();

  const onSubmit = handleSubmit(async (values) => {
    setError(null);
    try {
      await login(values.username, values.password);
      toast.success("登录成功");
      navigate("/", { replace: true });
    } catch (err) {
      setError(err instanceof Error ? err.message : "登录失败");
    }
  });

  return (
    <div className="relative flex min-h-screen flex-col">
      <div className="absolute right-4 top-4 z-10">
        <ThemeToggle />
      </div>

      <main className="mx-auto grid w-full max-w-6xl flex-1 items-center gap-12 px-6 py-16 lg:grid-cols-[1.1fr_0.9fr] lg:gap-20">
        {/* Left — brand statement + terminal */}
        <section className="space-y-8">
          <div className="space-y-4">
            <p className="mono-label text-[var(--color-accent)]">
              Git-backed documentation
            </p>
            <div className="flex items-center gap-3">
              <img src="/favicon.svg" alt="" className="size-12 rounded-xl" />
              <h1 className="font-display text-4xl font-semibold leading-[1.05] text-[var(--color-ink)] sm:text-5xl">
                XWiki
              </h1>
            </div>
            <p className="max-w-[42ch] text-[var(--color-ink-2)]">
              面向人类与 AI Agent 的轻量文档管理系统。一项目一 Git 仓库，
              文档即版本，ChangeSet 原子提交。
            </p>
          </div>
          <TerminalCard />
          <p className="mono-label text-[var(--color-ink-3)]">
            phase 01 · skeleton · serve / admin create
          </p>
        </section>

        {/* Right — sign-in panel */}
        <section className="hairline-panel p-8">
          <div className="mb-6 space-y-1">
            <h2 className="font-display text-xl font-semibold text-[var(--color-ink)]">
              登录以继续
            </h2>
            <p className="text-sm text-[var(--color-ink-3)]">
              使用管理员账号访问你的文档工作台
            </p>
          </div>

          <form onSubmit={onSubmit} className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="username">用户名</Label>
              <Input
                id="username"
                autoComplete="username"
                placeholder="admin"
                {...register("username")}
              />
              {errors.username && (
                <p className="text-sm text-[var(--color-destructive)]">
                  {errors.username.message}
                </p>
              )}
            </div>
            <div className="space-y-2">
              <Label htmlFor="password">密码</Label>
              <Input
                id="password"
                type="password"
                autoComplete="current-password"
                placeholder="••••••••"
                {...register("password")}
              />
              {errors.password && (
                <p className="text-sm text-[var(--color-destructive)]">
                  {errors.password.message}
                </p>
              )}
            </div>
            {error && (
              <Alert variant="destructive" role="alert">
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            )}
            <Button type="submit" className="w-full" disabled={isSubmitting}>
              {isSubmitting ? "登录中…" : "登录"}
            </Button>
          </form>

          <p className="mono-label mt-6 text-center text-[var(--color-ink-3)]">
            session · argon2id · http-only cookie
          </p>
        </section>
      </main>

      <footer className="border-t border-[var(--color-rule)]">
        <div className="mx-auto flex w-full max-w-6xl items-center justify-between px-6 py-4">
          <span className="mono-label text-[var(--color-ink-3)]">
            xwiki · git-backed documentation server
          </span>
          <span className="mono-label text-[var(--color-ink-3)]">v0.1</span>
        </div>
      </footer>
    </div>
  );
}
