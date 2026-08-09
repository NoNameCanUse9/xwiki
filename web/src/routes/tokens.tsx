import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { ArrowLeft, Copy, KeyRound, ShieldX } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { createToken, listTokens, revokeToken } from "@/lib/api/tokens";

export default function TokensPage() {
  const queryClient = useQueryClient();
  const { data, isLoading } = useQuery({
    queryKey: ["tokens"],
    queryFn: listTokens,
  });
  const [name, setName] = useState("");
  const [scope, setScope] = useState<"read" | "write">("write");
  const [projectIds, setProjectIds] = useState("");
  const [pathPrefixes, setPathPrefixes] = useState("docs");
  const [createdSecret, setCreatedSecret] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const onCreate = async () => {
    setBusy(true);
    try {
      const ids = projectIds
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean);
      const prefixes = pathPrefixes
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean);
      if (!name || ids.length === 0) {
        toast.error("名称与项目 ID 必填");
        return;
      }
      const res = await createToken({
        name,
        scope,
        project_ids: ids,
        path_prefixes: prefixes,
      });
      setCreatedSecret(res.secret);
      setName("");
      await queryClient.invalidateQueries({ queryKey: ["tokens"] });
      toast.success("Token 已创建，请立即复制明文");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "创建失败");
    } finally {
      setBusy(false);
    }
  };

  const onRevoke = async (id: string) => {
    if (!window.confirm("确认撤销该 Token？立即生效。")) return;
    try {
      await revokeToken(id);
      toast.success("已撤销");
      await queryClient.invalidateQueries({ queryKey: ["tokens"] });
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "撤销失败");
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
          settings · agent tokens
        </span>
      </header>

      <main className="flex-1 px-6 py-10 sm:px-10">
        <div className="mx-auto w-full max-w-3xl space-y-8">
          <div className="space-y-2">
            <p className="mono-label text-[var(--color-accent)]">settings</p>
            <h1 className="font-display text-3xl font-semibold leading-tight text-[var(--color-ink)]">
              Agent Token
            </h1>
            <p className="max-w-[58ch] text-[var(--color-ink-2)]">
              为 AI Agent 创建受限访问凭证：scope、项目绑定与路径前缀。
            </p>
          </div>

          <section className="hairline-panel space-y-4 p-6">
            <p className="mono-label text-[var(--color-ink-3)]">create token</p>
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="tok-name">名称</Label>
                <Input
                  id="tok-name"
                  placeholder="ci-bot"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="tok-scope">Scope</Label>
                <select
                  id="tok-scope"
                  value={scope}
                  onChange={(e) => setScope(e.target.value as "read" | "write")}
                  className="h-8 w-full rounded-[var(--radius)] border border-[var(--color-rule)] bg-[var(--color-paper)] px-3 text-sm text-[var(--color-ink)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]"
                >
                  <option value="read">read</option>
                  <option value="write">write</option>
                </select>
              </div>
            </div>
            <div className="space-y-2">
              <Label htmlFor="tok-projects">项目 ID（逗号分隔）</Label>
              <Input
                id="tok-projects"
                placeholder="prj_xxx, prj_yyy"
                value={projectIds}
                onChange={(e) => setProjectIds(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="tok-prefixes">写入路径前缀（逗号分隔）</Label>
              <Input
                id="tok-prefixes"
                placeholder="docs"
                value={pathPrefixes}
                onChange={(e) => setPathPrefixes(e.target.value)}
              />
            </div>
            <div className="flex justify-end">
              <Button onClick={() => void onCreate()} disabled={busy} className="gap-2">
                <KeyRound className="size-4" />
                {busy ? "创建中…" : "创建 Token"}
              </Button>
            </div>
            {createdSecret && (
              <div className="code-card space-y-2 p-4">
                <p className="mono-label text-white/50">
                  secret · 仅显示一次，请立即复制
                </p>
                <div className="flex items-center justify-between gap-3">
                  <code className="break-all text-sm text-[var(--color-accent)]">
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

          <section className="space-y-3">
            <p className="mono-label text-[var(--color-ink-3)]">
              tokens · {(data?.tokens ?? []).length}
            </p>
            {isLoading && <p className="mono-label text-[var(--color-ink-3)]">loading…</p>}
            <div className="hairline-panel divide-y divide-[var(--color-rule)] px-5">
              {(data?.tokens ?? []).length === 0 && !isLoading && (
                <p className="py-8 text-center text-sm text-[var(--color-ink-2)]">
                  还没有 Token
                </p>
              )}
              {(data?.tokens ?? []).map((t) => (
                <div key={t.id} className="flex items-center justify-between gap-3 py-3">
                  <div className="min-w-0">
                    <p className="truncate text-sm font-medium text-[var(--color-ink)]">
                      {t.name}
                      {t.revoked_at && (
                        <span className="mono-label ml-2 text-[var(--color-ink-3)]">
                          revoked
                        </span>
                      )}
                    </p>
                    <p className="mono-label mt-0.5 text-[var(--color-ink-3)]">
                      {t.scope} · {t.project_ids.length} 项目 · {t.path_prefixes.join(", ") || "—"}
                    </p>
                  </div>
                  {!t.revoked_at && (
                    <Button
                      variant="ghost"
                      size="sm"
                      className="shrink-0 gap-1.5 text-[var(--color-ink-3)]"
                      onClick={() => void onRevoke(t.id)}
                    >
                      <ShieldX className="size-3.5" />
                      撤销
                    </Button>
                  )}
                </div>
              ))}
            </div>
          </section>
        </div>
      </main>

      <footer className="border-t border-[var(--color-rule)] px-6 py-4">
        <p className="mono-label text-[var(--color-ink-3)]">
          agentdocs · phase 06 · tokens
        </p>
      </footer>
    </div>
  );
}
