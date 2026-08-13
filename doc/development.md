# 开发指南

## 前置

Go 1.26+、Node 26+、Git（运行时也需要 Git，阶段二起使用）。

## 常用命令

- 后端测试：go test ./...
- 前端测试：cd web && npm run test
- 前端构建：cd web && npm run build（必须先于 go build，产物被 go:embed）
- 编译：go build -o xwiki ./cmd/xwiki

## 新增迁移

在 internal/store/sqlite/migrations 增加 000NN_*.sql（含 `-- +goose Up` / `-- +goose Down` 注解），goose 在启动时自动执行。

## 占位符策略

web/dist/index.html 是提交到仓库的占位符，保证 fresh checkout 时 go:embed 可编译；真实产物由 npm run build 或 Docker 生成，不提交。

## 前端开发

npm run dev（:5173），/api 代理到 :8080。shadcn/ui 组件：npx shadcn@latest add <name>。

## 约定

- 所有 Git 命令必须经由 internal/gitrepo 封装（阶段二引入），业务层禁止直接 exec.Command
- 提交信息遵循 conventional commits（feat/fix/chore/docs）

## 编辑器（阶段：编辑器增强）

- 架构：Tiptap v3（ProseMirror）+ tiptap-markdown 0.9（md ↔ 编辑器双向桥接）
- 组件：`web/src/components/editor/rich-editor.tsx`（编辑器 + 工具栏 + slash 菜单 + BubbleMenu + 块操作 + 图片粘贴）
- 命令面板：`command-palette.tsx`（Cmd/Ctrl+K → 搜索跳转）
- 文件操作：`file-actions.tsx`（新建/删除/重命名，走 changesets API）
- 渲染增强：`markdown-render.ts`（highlight.js / KaTeX / mermaid 后处理）
- 扩展 Markdown：`internal/markdownx`（goldmark `:::info|warning|danger|details` 容器块）；wiki 链接 `[[path|label]]` 服务端重写
- 保存：编辑器 markdown → changeset（空 message → 后端默认「时间 操作者 修改 <path>」）；Cmd/Ctrl+S；未保存离开有确认
- 强制提交：每次保存必产生一个 commit（revision +1），保存失败草稿保留可重试
