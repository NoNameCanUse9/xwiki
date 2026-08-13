import { useState } from "react";
import { Link } from "react-router-dom";
import { ArrowLeft, KeyRound } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { changePassword } from "@/lib/api/auth";
import { useAuthStore } from "@/stores/auth";

export default function AccountPage() {
  const user = useAuthStore((s) => s.user);
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onSave = async () => {
    setError(null);
    if (newPassword.length < 8) {
      setError("新密码至少 8 位");
      return;
    }
    if (newPassword !== confirmPassword) {
      setError("两次输入的新密码不一致");
      return;
    }
    setBusy(true);
    try {
      await changePassword(currentPassword, newPassword);
      toast.success("密码已更新");
      setCurrentPassword("");
      setNewPassword("");
      setConfirmPassword("");
    } catch (err) {
      if (err instanceof Error) {
        setError(err.message);
      } else {
        setError("修改失败");
      }
    } finally {
      setBusy(false);
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
          settings · account
        </span>
      </header>

      <main className="flex-1 px-6 py-10 sm:px-10">
        <div className="mx-auto w-full max-w-2xl space-y-8">
          <div className="space-y-2">
            <p className="mono-label text-[var(--color-accent)]">settings</p>
            <h1 className="font-display text-3xl font-semibold leading-tight text-[var(--color-ink)]">
              账号设置
            </h1>
            <p className="max-w-[58ch] text-[var(--color-ink-2)]">
              查看账号信息，修改自己的登录密码。
            </p>
          </div>

          <section className="hairline-panel space-y-4 p-6">
            <p className="mono-label text-[var(--color-ink-3)]">account</p>
            <div className="space-y-1 border-b border-[var(--color-rule)] pb-4">
              <p className="font-display text-lg font-semibold text-[var(--color-ink)]">
                {user?.display_name || user?.username}
              </p>
              <p className="mono-label text-[var(--color-ink-3)]">
                {user?.username} · {user?.is_admin ? "admin" : "member"}
              </p>
            </div>
            <div className="space-y-2">
              <Label htmlFor="acc-current">当前密码</Label>
              <Input
                id="acc-current"
                type="password"
                autoComplete="current-password"
                value={currentPassword}
                onChange={(e) => setCurrentPassword(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="acc-new">新密码</Label>
              <Input
                id="acc-new"
                type="password"
                autoComplete="new-password"
                placeholder="至少 8 位"
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="acc-confirm">确认新密码</Label>
              <Input
                id="acc-confirm"
                type="password"
                autoComplete="new-password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
              />
            </div>
            {error && (
              <p className="text-sm text-[var(--color-destructive)]">{error}</p>
            )}
            <div className="flex justify-end pt-2">
              <Button onClick={() => void onSave()} disabled={busy} className="gap-2">
                <KeyRound className="size-4" />
                {busy ? "保存中…" : "保存"}
              </Button>
            </div>
          </section>
        </div>
      </main>

      <footer className="border-t border-[var(--color-rule)] px-6 py-4">
        <p className="mono-label text-[var(--color-ink-3)]">
          xwiki · account settings
        </p>
      </footer>
    </div>
  );
}
