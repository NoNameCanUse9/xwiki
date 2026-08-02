# 开发指南

## 前置

Go 1.26+、Node 26+、Git（运行时也需要 Git，阶段二起使用）。

## 常用命令

- 后端测试：go test ./...
- 前端测试：cd web && npm run test
- 前端构建：cd web && npm run build（必须先于 go build，产物被 go:embed）
- 编译：go build -o agentdocs ./cmd/agentdocs

## 新增迁移

在 internal/store/sqlite/migrations 增加 000NN_*.sql（含 `-- +goose Up` / `-- +goose Down` 注解），goose 在启动时自动执行。

## 占位符策略

web/dist/index.html 是提交到仓库的占位符，保证 fresh checkout 时 go:embed 可编译；真实产物由 npm run build 或 Docker 生成，不提交。

## 前端开发

npm run dev（:5173），/api 代理到 :8080。shadcn/ui 组件：npx shadcn@latest add <name>。

## 约定

- 所有 Git 命令必须经由 internal/gitrepo 封装（阶段二引入），业务层禁止直接 exec.Command
- 提交信息遵循 conventional commits（feat/fix/chore/docs）
