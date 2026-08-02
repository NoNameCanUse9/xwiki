import { Button } from "@/components/ui/button";
import { useAuthStore } from "@/stores/auth";

export default function HomePage() {
  const user = useAuthStore((s) => s.user);
  const logout = useAuthStore((s) => s.logout);
  return (
    <div className="flex min-h-screen flex-col items-center justify-center gap-4">
      <h1 className="text-2xl font-semibold">AgentDocs</h1>
      <p>已登录：{user?.username}</p>
      <Button variant="outline" onClick={() => void logout()}>
        退出登录
      </Button>
      <p className="text-sm text-muted-foreground">项目列表将在阶段二实现。</p>
    </div>
  );
}
