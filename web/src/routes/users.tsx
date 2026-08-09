import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import {
  ArrowLeft,
  Ban,
  CheckCircle2,
  KeyRound,
  Trash2,
  UserPlus,
  Users,
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
import {
  createUser,
  deleteUser,
  disableUser,
  enableUser,
  listUsers,
  resetUserPassword,
} from "@/lib/api/users";
import { ApiError } from "@/lib/api/client";
import type { UserView } from "@/lib/api/users";

export default function UsersPage() {
  const queryClient = useQueryClient();
  const { data, isLoading } = useQuery({ queryKey: ["users"], queryFn: listUsers });
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [isAdmin, setIsAdmin] = useState(false);
  const [busy, setBusy] = useState(false);

  const onCreate = async () => {
    setBusy(true);
    try {
      await createUser({
        username,
        password,
        display_name: displayName || undefined,
        is_admin: isAdmin,
      });
      toast.success(`用户 ${username} 已创建`);
      setUsername("");
      setPassword("");
      setDisplayName("");
      setIsAdmin(false);
      await queryClient.invalidateQueries({ queryKey: ["users"] });
    } catch (err) {
      if (err instanceof ApiError && err.status === 409) {
        toast.error("用户名已存在");
      } else {
        toast.error(err instanceof Error ? err.message : "创建失败");
      }
    } finally {
      setBusy(false);
    }
  };

  const onToggle = async (id: string, disabled: boolean) => {
    try {
      if (disabled) {
        await enableUser(id);
        toast.success("已启用");
      } else {
        await disableUser(id);
        toast.success("已禁用");
      }
      await queryClient.invalidateQueries({ queryKey: ["users"] });
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "操作失败");
    }
  };

  const [resetTarget, setResetTarget] = useState<UserView | null>(null);
  const [resetValue, setResetValue] = useState("");
  const [resetBusy, setResetBusy] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<UserView | null>(null);
  const [deleteBusy, setDeleteBusy] = useState(false);

  const openReset = (u: UserView) => {
    setResetValue("");
    setResetTarget(u);
  };

  const onReset = async () => {
    if (!resetTarget) return;
    if (resetValue.length < 8) {
      toast.error("密码至少 8 位");
      return;
    }
    setResetBusy(true);
    try {
      await resetUserPassword(resetTarget.id, resetValue);
      toast.success(`${resetTarget.display_name || resetTarget.username} 的密码已重置`);
      setResetTarget(null);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "重置失败");
    } finally {
      setResetBusy(false);
    }
  };

  const onDelete = async () => {
    if (!deleteTarget) return;
    setDeleteBusy(true);
    try {
      await deleteUser(deleteTarget.id);
      toast.success(`用户 ${deleteTarget.username} 已删除`);
      setDeleteTarget(null);
      await queryClient.invalidateQueries({ queryKey: ["users"] });
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "删除失败");
    } finally {
      setDeleteBusy(false);
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
          settings · users
        </span>
      </header>

      <main className="flex-1 px-6 py-10 sm:px-10">
        <div className="mx-auto w-full max-w-3xl space-y-8">
          <div className="space-y-2">
            <p className="mono-label text-[var(--color-accent)]">settings</p>
            <h1 className="font-display text-3xl font-semibold leading-tight text-[var(--color-ink)]">
              用户管理
            </h1>
            <p className="max-w-[58ch] text-[var(--color-ink-2)]">
              创建成员账号、分配管理员角色、禁用离职账号。禁用后无法登录。
            </p>
          </div>

          <section className="hairline-panel space-y-4 p-6">
            <p className="mono-label text-[var(--color-ink-3)]">create user</p>
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="u-name">用户名</Label>
                <Input
                  id="u-name"
                  placeholder="alice"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="u-pass">密码（至少 8 位）</Label>
                <Input
                  id="u-pass"
                  type="password"
                  placeholder="••••••••"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="u-display">显示名</Label>
                <Input
                  id="u-display"
                  placeholder="Alice"
                  value={displayName}
                  onChange={(e) => setDisplayName(e.target.value)}
                />
              </div>
              <label className="flex cursor-pointer items-center gap-2 pt-7 text-sm text-[var(--color-ink-2)]">
                <input
                  type="checkbox"
                  checked={isAdmin}
                  onChange={(e) => setIsAdmin(e.target.checked)}
                  className="size-4 accent-[var(--color-accent)]"
                />
                管理员
              </label>
            </div>
            <div className="flex justify-end">
              <Button onClick={() => void onCreate()} disabled={busy} className="gap-2">
                <UserPlus className="size-4" />
                {busy ? "创建中…" : "创建用户"}
              </Button>
            </div>
          </section>

          <section className="space-y-3">
            <p className="mono-label text-[var(--color-ink-3)]">
              users · {(data?.users ?? []).length}
            </p>
            {isLoading && <p className="mono-label text-[var(--color-ink-3)]">loading…</p>}
            <div className="hairline-panel divide-y divide-[var(--color-rule)] px-5">
              {data?.users.map((u) => (
                <div key={u.id} className="flex items-center justify-between gap-3 py-3">
                  <div className="flex min-w-0 items-center gap-3">
                    <Users className="size-4 shrink-0 text-[var(--color-ink-3)]" />
                    <div className="min-w-0">
                      <p className="truncate text-sm font-medium text-[var(--color-ink)]">
                        {u.display_name || u.username}
                        {u.is_admin && (
                          <span className="mono-label ml-2 text-[var(--color-accent)]">
                            admin
                          </span>
                        )}
                        {u.disabled && (
                          <span className="mono-label ml-2 text-[var(--color-destructive)]">
                            disabled
                          </span>
                        )}
                      </p>
                      <p className="mono-label mt-0.5 text-[var(--color-ink-3)]">
                        {u.username}
                      </p>
                    </div>
                  </div>
                  {u.username !== "admin" && (
                    <div className="flex shrink-0 items-center gap-1">
                      <Button
                        variant="ghost"
                        size="sm"
                        className="gap-1.5 text-[var(--color-ink-3)]"
                        onClick={() => openReset(u)}
                      >
                        <KeyRound className="size-3.5" />
                        重置密码
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="gap-1.5 text-[var(--color-destructive)]"
                        onClick={() => setDeleteTarget(u)}
                      >
                        <Trash2 className="size-3.5" />
                        删除
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="gap-1.5 text-[var(--color-ink-3)]"
                        onClick={() => void onToggle(u.id, u.disabled)}
                      >
                        {u.disabled ? (
                          <>
                            <CheckCircle2 className="size-3.5" />
                            启用
                          </>
                        ) : (
                          <>
                            <Ban className="size-3.5" />
                            禁用
                          </>
                        )}
                      </Button>
                    </div>
                  )}
                </div>
              ))}
            </div>
          </section>
        </div>
      </main>

      <footer className="border-t border-[var(--color-rule)] px-6 py-4">
        <p className="mono-label text-[var(--color-ink-3)]">
          agentdocs · user management
        </p>
      </footer>

      <Dialog open={resetTarget !== null} onOpenChange={(v) => !v && setResetTarget(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>重置密码</DialogTitle>
            <DialogDescription>
              为 {resetTarget?.display_name || resetTarget?.username} 设置新密码，重置后其所有会话将失效。
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <Label htmlFor="reset-pass">新密码</Label>
            <Input
              id="reset-pass"
              type="password"
              placeholder="至少 8 位"
              value={resetValue}
              onChange={(e) => setResetValue(e.target.value)}
              autoFocus
            />
          </div>
          <div className="flex justify-end gap-2 pt-2">
            <Button
              type="button"
              variant="outline"
              onClick={() => setResetTarget(null)}
              disabled={resetBusy}
            >
              取消
            </Button>
            <Button type="button" onClick={() => void onReset()} disabled={resetBusy}>
              {resetBusy ? "保存中…" : "保存"}
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      <Dialog open={deleteTarget !== null} onOpenChange={(v) => !v && setDeleteTarget(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>删除用户</DialogTitle>
            <DialogDescription>
              确认删除用户「{deleteTarget?.username}」？其账号、会话将被移除，无法恢复。
            </DialogDescription>
          </DialogHeader>
          <div className="flex justify-end gap-2 pt-2">
            <Button
              type="button"
              variant="outline"
              onClick={() => setDeleteTarget(null)}
              disabled={deleteBusy}
            >
              取消
            </Button>
            <Button
              type="button"
              variant="destructive"
              onClick={() => void onDelete()}
              disabled={deleteBusy}
            >
              {deleteBusy ? "删除中…" : "确认删除"}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
