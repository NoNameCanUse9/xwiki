import { Link, Outlet } from "react-router-dom";
import { ShieldX } from "lucide-react";
import { useAuthStore } from "@/stores/auth";

/** 仅管理员可访问的布局路由；非管理员显示无权限提示。 */
export default function AdminRoute() {
  const user = useAuthStore((s) => s.user);

  if (!user?.is_admin) {
    return (
      <div className="flex min-h-screen flex-col items-center justify-center gap-4 px-6 text-center">
        <ShieldX className="size-10 text-[var(--color-ink-3)]" />
        <div className="space-y-1">
          <p className="font-display text-xl font-semibold text-[var(--color-ink)]">
            无权限访问
          </p>
          <p className="text-sm text-[var(--color-ink-2)]">
            该页面仅管理员可用。如需管理项目成员或凭证，请联系管理员。
          </p>
        </div>
        <Link
          to="/"
          className="mono-label rounded-[var(--radius)] border border-[var(--color-rule)] px-3 py-1.5 text-[var(--color-ink-2)] transition-colors hover:border-[var(--color-rule-2)] hover:text-[var(--color-accent)]"
        >
          ← 返回工作区
        </Link>
      </div>
    );
  }

  return <Outlet />;
}
