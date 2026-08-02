import { useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Download, Paperclip, Trash2, Upload } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { getRevision, submitChangeset } from "@/lib/api/changesets";
import { getTree } from "@/lib/api/docs";

export function attachmentUrl(projectId: string, path: string): string {
  return `/api/v1/projects/${encodeURIComponent(projectId)}/attachments/${path}`;
}

const MAX_ATTACHMENT = 5 * 1024 * 1024; // 5 MiB

export default function AttachmentsPanel({ projectId }: { projectId: string }) {
  const queryClient = useQueryClient();
  const fileRef = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);

  const { data, isLoading } = useQuery({
    queryKey: ["attachments", projectId],
    queryFn: () => getTree(projectId, "attachments"),
    enabled: projectId.length > 0,
  });
  const files = (data?.tree ?? []).filter((e) => e.type === "blob");

  const upload = async (file: File) => {
    if (file.size > MAX_ATTACHMENT) {
      toast.error("附件超过 5 MiB");
      return;
    }
    setBusy(true);
    try {
      const reader = new FileReader();
      const base64 = await new Promise<string>((resolve, reject) => {
        reader.onload = () => {
          const result = reader.result as string;
          resolve(result.slice(result.indexOf(",") + 1));
        };
        reader.onerror = () => reject(new Error("读取文件失败"));
        reader.readAsDataURL(file);
      });
      const rev = await getRevision(projectId);
      await submitChangeset(projectId, {
        base_revision: rev.revision,
        message: "",
        changes: [
          {
            op: "create",
            path: `attachments/${file.name}`,
            content: base64,
            encoding: "base64" as const,
          },
        ],
      });
      toast.success(`已上传 ${file.name}`);
      await queryClient.invalidateQueries({ queryKey: ["attachments"] });
      await queryClient.invalidateQueries({ queryKey: ["tree"] });
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "上传失败");
    } finally {
      setBusy(false);
      if (fileRef.current) fileRef.current.value = "";
    }
  };

  const remove = async (path: string) => {
    if (!window.confirm(`确认删除附件 ${path}？`)) return;
    try {
      const rev = await getRevision(projectId);
      await submitChangeset(projectId, {
        base_revision: rev.revision,
        message: "",
        changes: [{ op: "delete", path }],
      });
      toast.success("已删除");
      await queryClient.invalidateQueries({ queryKey: ["attachments"] });
      await queryClient.invalidateQueries({ queryKey: ["tree"] });
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "删除失败");
    }
  };

  return (
    <section className="space-y-3">
      <div className="flex items-center justify-between">
        <p className="mono-label flex items-center gap-2 text-[var(--color-ink-3)]">
          <Paperclip className="size-3.5" />
          attachments · {files.length}
        </p>
        <Button size="sm" variant="outline" className="gap-2" disabled={busy} onClick={() => fileRef.current?.click()}>
          <Upload className="size-3.5" />
          上传附件
        </Button>
        <input
          ref={fileRef}
          type="file"
          aria-label="上传附件"
          className="hidden"
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) void upload(f);
          }}
        />
      </div>
      {isLoading && <p className="mono-label text-[var(--color-ink-3)]">loading…</p>}
      {!isLoading && files.length === 0 && (
        <p className="hairline-panel px-4 py-6 text-center text-sm text-[var(--color-ink-2)]">
          还没有附件，上传图片、PDF 等文件供页面引用。
        </p>
      )}
      {files.length > 0 && (
        <div className="hairline-panel divide-y divide-[var(--color-rule)] px-4">
          {files.map((f) => (
            <div key={f.path} className="flex items-center justify-between gap-3 py-2.5">
              <a
                href={attachmentUrl(projectId, f.path)}
                target="_blank"
                rel="noreferrer"
                className="min-w-0 truncate font-mono text-xs text-[var(--color-accent)] hover:underline"
              >
                {f.name}
              </a>
              <span className="flex shrink-0 items-center gap-1">
                <a
                  href={attachmentUrl(projectId, f.path)}
                  target="_blank"
                  rel="noreferrer"
                  aria-label={`下载 ${f.name}`}
                  className="flex h-6 w-6 items-center justify-center rounded-sm text-[var(--color-ink-3)] hover:bg-[var(--color-surface-accent)]"
                >
                  <Download className="size-3" />
                </a>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-6 w-6 p-0 text-[var(--color-destructive)]"
                  aria-label={`删除 ${f.name}`}
                  onClick={() => void remove(f.path)}
                >
                  <Trash2 className="size-3" />
                </Button>
              </span>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
