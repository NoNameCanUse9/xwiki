import { useState } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { toast } from "sonner";
import { Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { ApiError } from "@/lib/api/client";
import { createProject } from "@/lib/api/projects";
import type { Project } from "@/lib/api/types";

const schema = z.object({
  name: z
    .string()
    .min(1, "请输入项目名")
    .max(64, "项目名最长 64 字符")
    .regex(/^[a-z0-9]+(-[a-z0-9]+)*$/, "小写字母、数字和单个连字符"),
  description: z.string().max(500, "描述最长 500 字符").optional(),
});

type FormValues = z.infer<typeof schema>;

interface Props {
  onCreated: (project: Project) => void;
}

export default function ProjectCreateDialog({ onCreated }: Props) {
  const [open, setOpen] = useState(false);
  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: { name: "", description: "" },
  });

  const onSubmit = handleSubmit(async (values) => {
    try {
      const res = await createProject({ name: values.name, description: values.description ?? "" });
      toast.success(`项目 ${res.project.name} 已创建`);
      reset();
      setOpen(false);
      onCreated(res.project);
    } catch (err) {
      if (err instanceof ApiError && err.code === "project_name_conflict") {
        toast.error("同名项目已存在");
      } else {
        toast.error(err instanceof Error ? err.message : "创建失败");
      }
    }
  });

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button className="gap-2">
          <Plus className="size-4" />
          新建项目
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>新建项目</DialogTitle>
          <DialogDescription>
            每个项目拥有独立的 Git 仓库，文档即版本。
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={onSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="project-name">项目名</Label>
            <Input
              id="project-name"
              placeholder="docs-site"
              autoComplete="off"
              {...register("name")}
            />
            {errors.name && (
              <p className="text-sm text-[var(--color-destructive)]">
                {errors.name.message}
              </p>
            )}
          </div>
          <div className="space-y-2">
            <Label htmlFor="project-description">描述</Label>
            <Input
              id="project-description"
              placeholder="产品文档（可选）"
              {...register("description")}
            />
            {errors.description && (
              <p className="text-sm text-[var(--color-destructive)]">
                {errors.description.message}
              </p>
            )}
          </div>
          <div className="flex justify-end gap-2 pt-2">
            <Button
              type="button"
              variant="outline"
              onClick={() => setOpen(false)}
            >
              取消
            </Button>
            <Button type="submit" disabled={isSubmitting}>
              {isSubmitting ? "创建中…" : "创建"}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
