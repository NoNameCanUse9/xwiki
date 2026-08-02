# AgentDocs 阶段一：项目骨架 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 搭建 AgentDocs 的可运行骨架：Go 服务 + React 前端 + SQLite，支持创建管理员、登录、会话在服务重启后保持有效，前端构建产物嵌入 Go 二进制。

**Architecture:** 单体 Go 服务（chi 路由 + SQLite 存储 + 领域服务层 + HTTP 层），前端为 Vite+React SPA，构建产物经 `go:embed` 打进二进制；网页会话用 HttpOnly Cookie + 服务端 `sessions` 表（只存哈希）；CLI 与 HTTP 共用同一套 app 装配，杜绝两套逻辑。

**Tech Stack:** Go 1.26 / chi v5 / modernc.org/sqlite（纯 Go 无 CGO）/ goose v3 迁移 / Argon2id / ULID；React 19 / TypeScript / Vite / Tailwind CSS v4 / shadcn/ui / React Router v7 / TanStack Query / Zustand / React Hook Form / Zod / Vitest + React Testing Library；Docker + Docker Compose。

---

## 0. 范围（本计划 = spec 阶段一）

**做：**

- Go 服务骨架（`cmd/agentdocs`：`serve`、`admin create`）
- 配置（环境变量 + 子命令 flag）
- SQLite + goose 迁移（`users`、`sessions`、`schema_migrations`）
- 认证：Argon2id 密码哈希、HttpOnly Session Cookie
- API：`/healthz`、`/readyz`、`/api/v1/auth/login|logout|me|password`
- 统一错误信封、结构化 JSON 日志（每个请求带 `request_id`）
- React + shadcn/ui 骨架、登录页、路由守卫、退出登录
- 前端构建产物嵌入二进制；Dockerfile、docker-compose.yml、.dockerignore
- 文档：README、`doc/architecture.md`、`doc/api.md`、`doc/development.md`

**不做（后续阶段）：** 项目/仓库、Git 操作、文档树、ChangeSet 写入、历史/Diff、Agent Token、搜索、OpenAPI、导入导出、注册流程、Playwright E2E。

**验收标准（spec §27 阶段一）：**

1. 可以创建管理员（CLI）。
2. 可以登录（网页 + API）。
3. 服务重启后 Session 和数据库正常。

## 1. 文件结构（本阶段创建/修改）

```text
xwiki/
├── .gitignore                        （修改：忽略 data/、二进制）
├── README.md                         （Task 18）
├── go.mod / go.sum                   （Task 2）
├── cmd/agentdocs/main.go             （CLI：serve / admin create）
├── internal/
│   ├── app/app.go                    （装配 + CreateAdmin + Run）
│   ├── config/config.go              （环境变量配置）
│   ├── auth/
│   │   ├── password.go               （Argon2id 哈希/校验）
│   │   └── service.go                （登录 + 会话 CRUD）
│   ├── user/store.go                 （users 存储）
│   ├── httpapi/
│   │   ├── request/request.go        （RequestID ctx + DecodeJSON）
│   │   ├── response/response.go      （统一 JSON 错误信封）
│   │   ├── middleware/               （RequestID/Logger/Recoverer/SessionAuth）
│   │   └── handlers/auth.go          （/auth/* 处理器）
│   ├── server/router.go              （路由 + SPA 静态服务）
│   ├── store/sqlite/                 （Open + 迁移 + migrations/*.sql）
│   └── platform/
│       ├── id/id.go                  （带前缀 ULID）
│       └── clock/clock.go            （可测试时钟）
├── web/
│   ├── embed.go                      （go:embed dist）
│   ├── dist/index.html               （占位符，保证 fresh checkout 可编译）
│   ├── src/                          （前端源码，Task 14-17）
│   └── package.json 等
├── doc/
│   ├── spec.md                       （不变）
│   ├── architecture.md / api.md / development.md   （Task 18）
│   └── plans/                        （本计划与索引）
├── Dockerfile / .dockerignore / docker-compose.yml  （Task 19）
└── data/                             （运行时生成，不入库）
```

## 2. 任务

### Task 1: 初始化 Git 仓库与根目录文件

**Files:**
- Modify: `.gitignore`

- [ ] **Step 1: 初始化 Git 仓库**

```bash
cd /home/choken/code/xwiki
git init
git branch -m main
```

Expected: `Initialized empty Git repository in /home/choken/code/xwiki/.git/`，当前分支 `main`。

- [ ] **Step 2: 更新根 .gitignore**

将 `.gitignore` 整体替换为：

```gitignore
# Local Pi runtime state
.atl/

# AgentDocs runtime
/data/
/agentdocs
*.log
```

- [ ] **Step 3: 首次提交（含 spec 与本计划）**

```bash
git add .gitignore doc/
git commit -m "chore: initialize agentdocs repository"
```

Expected: 提交包含 `doc/spec.md`、`doc/plans/README.md`、本计划文件。

### Task 2: Go Module 与依赖

**Files:**
- Create: `go.mod`、`go.sum`

- [ ] **Step 1: 初始化模块**

```bash
go mod init agentdocs
```

Expected: 生成 `go.mod`，module 为 `agentdocs`。注：模块路径暂用 `agentdocs`；将来托管 GitHub 时整体机械替换即可。

- [ ] **Step 2: 安装依赖**

```bash
go get github.com/go-chi/chi/v5@latest
go get github.com/go-chi/cors@latest
go get github.com/oklog/ulid/v2@latest
go get github.com/pressly/goose/v3@latest
go get modernc.org/sqlite@latest
go get golang.org/x/crypto@latest
```

Expected: 全部成功，生成 `go.sum`。`golang-jwt` 留到阶段六（Agent Token）再引入。

- [ ] **Step 3: 提交**

```bash
git add go.mod go.sum
git commit -m "chore: add go module and dependencies"
```

### Task 3: 配置包 internal/config（TDD）

**Files:**
- Create: `internal/config/config.go`
- Test: `internal/config/config_test.go`

- [ ] **Step 1: 写失败测试**

`internal/config/config_test.go`：

```go
package config

import "testing"

func TestEnvOr(t *testing.T) {
	t.Setenv("AGENTDOCS_TEST_X", "value")
	if got := envOr("AGENTDOCS_TEST_X", "default"); got != "value" {
		t.Fatalf("envOr = %q, want %q", got, "value")
	}
	t.Setenv("AGENTDOCS_TEST_X", "")
	if got := envOr("AGENTDOCS_TEST_X", "default"); got != "default" {
		t.Fatalf("envOr = %q, want %q", got, "default")
	}
}

func TestLoadDefaults(t *testing.T) {
	t.Setenv("AGENTDOCS_DATA_DIR", "")
	t.Setenv("AGENTDOCS_HTTP_ADDR", "")
	cfg := Load()
	if cfg.DataDir != "data" || cfg.HTTPAddr != ":8080" {
		t.Fatalf("unexpected defaults: %+v", cfg)
	}
	if cfg.SessionTTL <= 0 || cfg.MaxBodyBytes <= 0 {
		t.Fatalf("unexpected defaults: %+v", cfg)
	}
}

func TestLoadFromEnv(t *testing.T) {
	t.Setenv("AGENTDOCS_HTTP_ADDR", ":9090")
	t.Setenv("AGENTDOCS_SESSION_TTL", "1h")
	cfg := Load()
	if cfg.HTTPAddr != ":9090" {
		t.Fatalf("HTTPAddr = %q", cfg.HTTPAddr)
	}
	if cfg.SessionTTL.String() != "1h0m0s" {
		t.Fatalf("SessionTTL = %v", cfg.SessionTTL)
	}
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
go test ./internal/config/
```

Expected: FAIL，报 package 不存在（编译错误）。

- [ ] **Step 3: 实现配置包**

`internal/config/config.go`：

```go
package config

import (
	"os"
	"strconv"
	"time"
)

type Config struct {
	DataDir       string
	HTTPAddr      string
	WebOrigin     string
	SessionTTL    time.Duration
	MaxBodyBytes  int64
	SecureCookies bool
}

// Load reads configuration from AGENTDOCS_* environment variables,
// falling back to development-friendly defaults.
func Load() *Config {
	return &Config{
		DataDir:       envOr("AGENTDOCS_DATA_DIR", "data"),
		HTTPAddr:      envOr("AGENTDOCS_HTTP_ADDR", ":8080"),
		WebOrigin:     envOr("AGENTDOCS_WEB_ORIGIN", "http://localhost:5173"),
		SessionTTL:    envDuration("AGENTDOCS_SESSION_TTL", 30*24*time.Hour),
		MaxBodyBytes:  envInt64("AGENTDOCS_MAX_BODY_BYTES", 1<<20),
		SecureCookies: envBool("AGENTDOCS_COOKIE_SECURE", false),
	}
}

func envOr(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

func envDuration(key string, def time.Duration) time.Duration {
	if v := os.Getenv(key); v != "" {
		if d, err := time.ParseDuration(v); err == nil {
			return d
		}
	}
	return def
}

func envInt64(key string, def int64) int64 {
	if v := os.Getenv(key); v != "" {
		if n, err := strconv.ParseInt(v, 10, 64); err == nil {
			return n
		}
	}
	return def
}

func envBool(key string, def bool) bool {
	if v := os.Getenv(key); v != "" {
		if b, err := strconv.ParseBool(v); err == nil {
			return b
		}
	}
	return def
}
```

- [ ] **Step 4: 运行测试确认通过**

```bash
go test ./internal/config/
```

Expected: `ok  agentdocs/internal/config`。

- [ ] **Step 5: 格式检查 + 提交**

```bash
gofmt -l .          # 期望：无输出
git add internal/config/
git commit -m "feat: add config package with env loading"
```

### Task 4: 平台工具 id + clock（TDD）

**Files:**
- Create: `internal/platform/id/id.go`、`internal/platform/clock/clock.go`
- Test: `internal/platform/id/id_test.go`

- [ ] **Step 1: 写失败测试**

`internal/platform/id/id_test.go`：

```go
package id

import "testing"

func TestNew(t *testing.T) {
	got := New("usr")
	// ULID 为 26 字符，前缀加下划线共 30 字符
	if len(got) != 30 {
		t.Fatalf("unexpected id %q (len %d)", got, len(got))
	}
	if got[:4] != "usr_" {
		t.Fatalf("prefix missing: %q", got)
	}
}

func TestNewUnique(t *testing.T) {
	a, b := New("prj"), New("prj")
	if a == b {
		t.Fatalf("ids must differ: %q", a)
	}
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
go test ./internal/platform/id/
```

Expected: FAIL，package 不存在。

- [ ] **Step 3: 实现 id 与 clock**

`internal/platform/id/id.go`：

```go
package id

import "github.com/oklog/ulid/v2"

// New returns a prefixed ULID, e.g. New("usr") -> "usr_01KABC...".
func New(prefix string) string {
	return prefix + "_" + ulid.Make().String()
}
```

`internal/platform/clock/clock.go`：

```go
package clock

import "time"

// Clock abstracts time so services are testable with a fake clock.
type Clock interface {
	Now() time.Time
}

type Real struct{}

func (Real) Now() time.Time { return time.Now() }
```

- [ ] **Step 4: 运行测试确认通过**

```bash
go test ./internal/platform/...
```

Expected: `ok  agentdocs/internal/platform/id`。

- [ ] **Step 5: 提交**

```bash
gofmt -l .
git add internal/platform/
git commit -m "feat: add id and clock platform packages"
```

### Task 5: SQLite 存储与迁移（TDD）

**Files:**
- Create: `internal/store/sqlite/sqlite.go`
- Create: `internal/store/sqlite/migrations/00001_init.sql`
- Test: `internal/store/sqlite/sqlite_test.go`

- [ ] **Step 1: 写失败测试**

`internal/store/sqlite/sqlite_test.go`：

```go
package sqlite

import (
	"path/filepath"
	"testing"
)

func TestOpenRunsMigrations(t *testing.T) {
	dir := t.TempDir()
	db, err := Open(dir)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	for _, name := range []string{"users", "sessions", "schema_migrations"} {
		var n int
		if err := db.QueryRow(
			`SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?`, name,
		).Scan(&n); err != nil {
			t.Fatalf("query %s: %v", name, err)
		}
		if n != 1 {
			t.Fatalf("table %s missing (n=%d)", name, n)
		}
	}
	db.Close()

	// Reopening the same directory must be idempotent.
	db2, err := Open(dir)
	if err != nil {
		t.Fatalf("second Open: %v", err)
	}
	db2.Close()
}

func TestOpenCreatesNestedDataDir(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "nested", "data")
	db, err := Open(dir)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	db.Close()
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
go test ./internal/store/sqlite/
```

Expected: FAIL，package 不存在。

- [ ] **Step 3: 编写迁移**

`internal/store/sqlite/migrations/00001_init.sql`：

```sql
-- 00001_init.sql: users and sessions

CREATE TABLE users (
    id            TEXT PRIMARY KEY,
    username      TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    display_name  TEXT NOT NULL DEFAULT '',
    is_admin      INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE sessions (
    id           TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash   TEXT NOT NULL UNIQUE,
    expires_at   TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    last_used_at TEXT NOT NULL
);

CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);
```

- [ ] **Step 4: 实现 sqlite.Open**

`internal/store/sqlite/sqlite.go`：

```go
package sqlite

import (
	"database/sql"
	"embed"
	"fmt"
	"os"
	"path/filepath"

	"github.com/pressly/goose/v3"
	_ "modernc.org/sqlite"
)

//go:embed migrations/*.sql
var migrationsFS embed.FS

// Open creates the data directory if needed, opens the SQLite database with
// WAL + foreign keys + busy timeout, and applies pending migrations.
func Open(dataDir string) (*sql.DB, error) {
	if err := os.MkdirAll(dataDir, 0o755); err != nil {
		return nil, fmt.Errorf("create data dir: %w", err)
	}
	db, err := sql.Open("sqlite", filepath.Join(dataDir, "agentdocs.db"))
	if err != nil {
		return nil, fmt.Errorf("open sqlite: %w", err)
	}
	db.SetMaxOpenConns(1)
	for _, pragma := range []string{
		"PRAGMA journal_mode=WAL",
		"PRAGMA foreign_keys=ON",
		"PRAGMA busy_timeout=5000",
	} {
		if _, err := db.Exec(pragma); err != nil {
			db.Close()
			return nil, fmt.Errorf("apply %s: %w", pragma, err)
		}
	}
	goose.SetBaseFS(migrationsFS)
	goose.SetDialect("sqlite3")
	goose.SetTableName("schema_migrations")
	if err := goose.Up(db, "migrations"); err != nil {
		db.Close()
		return nil, fmt.Errorf("run migrations: %w", err)
	}
	return db, nil
}
```

注：`goose.SetTableName("schema_migrations")` 用于匹配 spec §12 的表名。若当前 goose 版本没有该函数（编译报错），删除该行，改用 goose 默认表 `goose_db_version` 并在 Task 18 文档中注明。

- [ ] **Step 5: 运行测试确认通过**

```bash
go test ./internal/store/sqlite/
```

Expected: `ok  agentdocs/internal/store/sqlite`。

- [ ] **Step 6: 提交**

```bash
gofmt -l .
git add internal/store/sqlite/
git commit -m "feat: add sqlite store with goose migrations"
```

### Task 6: 用户存储 internal/user（TDD）

**Files:**
- Create: `internal/user/store.go`
- Test: `internal/user/store_test.go`

- [ ] **Step 1: 写失败测试**

`internal/user/store_test.go`：

```go
package user

import (
	"context"
	"errors"
	"testing"
	"time"

	"agentdocs/internal/store/sqlite"
)

func newStore(t *testing.T) *Store {
	t.Helper()
	db, err := sqlite.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	return NewStore(db)
}

func TestCreateAndGetByUsername(t *testing.T) {
	s := newStore(t)
	now := time.Now().UTC()
	u := &User{ID: "usr_1", Username: "admin", DisplayName: "Admin",
		PasswordHash: "hash", IsAdmin: true, CreatedAt: now, UpdatedAt: now}
	if err := s.Create(context.Background(), u); err != nil {
		t.Fatalf("Create: %v", err)
	}
	got, err := s.GetByUsername(context.Background(), "admin")
	if err != nil {
		t.Fatalf("GetByUsername: %v", err)
	}
	if got.ID != "usr_1" || got.Username != "admin" || !got.IsAdmin {
		t.Fatalf("unexpected user: %+v", got)
	}
	if _, err := s.GetByUsername(context.Background(), "nobody"); !errors.Is(err, ErrNotFound) {
		t.Fatalf("want ErrNotFound, got %v", err)
	}
}

func TestCreateDuplicateUsername(t *testing.T) {
	s := newStore(t)
	now := time.Now().UTC()
	u := &User{ID: "usr_1", Username: "admin", DisplayName: "Admin",
		PasswordHash: "hash", CreatedAt: now, UpdatedAt: now}
	if err := s.Create(context.Background(), u); err != nil {
		t.Fatal(err)
	}
	u2 := *u
	u2.ID = "usr_2"
	if err := s.Create(context.Background(), &u2); err == nil {
		t.Fatal("duplicate username allowed")
	}
}

func TestUpdatePassword(t *testing.T) {
	s := newStore(t)
	now := time.Now().UTC()
	u := &User{ID: "usr_1", Username: "admin", DisplayName: "Admin",
		PasswordHash: "old", CreatedAt: now, UpdatedAt: now}
	if err := s.Create(context.Background(), u); err != nil {
		t.Fatal(err)
	}
	if err := s.UpdatePassword(context.Background(), "usr_1", "new"); err != nil {
		t.Fatalf("UpdatePassword: %v", err)
	}
	got, err := s.GetByUsername(context.Background(), "admin")
	if err != nil {
		t.Fatal(err)
	}
	if got.PasswordHash != "new" {
		t.Fatalf("password hash not updated: %q", got.PasswordHash)
	}
	if err := s.UpdatePassword(context.Background(), "usr_404", "x"); !errors.Is(err, ErrNotFound) {
		t.Fatalf("want ErrNotFound, got %v", err)
	}
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
go test ./internal/user/
```

Expected: FAIL，package 不存在。

- [ ] **Step 3: 实现用户存储**

`internal/user/store.go`：

```go
package user

import (
	"context"
	"database/sql"
	"errors"
	"time"
)

var ErrNotFound = errors.New("user not found")

type User struct {
	ID           string
	Username     string
	DisplayName  string
	PasswordHash string
	IsAdmin      bool
	CreatedAt    time.Time
	UpdatedAt    time.Time
}

type Store struct {
	db *sql.DB
}

func NewStore(db *sql.DB) *Store {
	return &Store{db: db}
}

func (s *Store) Create(ctx context.Context, u *User) error {
	_, err := s.db.ExecContext(ctx, `
		INSERT INTO users (id, username, password_hash, display_name, is_admin, created_at, updated_at)
		VALUES (?, ?, ?, ?, ?, ?, ?)`,
		u.ID, u.Username, u.PasswordHash, u.DisplayName, u.IsAdmin,
		u.CreatedAt.UTC().Format(time.RFC3339), u.UpdatedAt.UTC().Format(time.RFC3339))
	return err
}

func (s *Store) GetByUsername(ctx context.Context, username string) (*User, error) {
	row := s.db.QueryRowContext(ctx, `
		SELECT id, username, password_hash, display_name, is_admin, created_at, updated_at
		FROM users WHERE username = ?`, username)
	return scanUser(row)
}

func (s *Store) UpdatePassword(ctx context.Context, id, passwordHash string) error {
	res, err := s.db.ExecContext(ctx, `
		UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?`,
		passwordHash, time.Now().UTC().Format(time.RFC3339), id)
	if err != nil {
		return err
	}
	n, err := res.RowsAffected()
	if err != nil {
		return err
	}
	if n == 0 {
		return ErrNotFound
	}
	return nil
}

func scanUser(row *sql.Row) (*User, error) {
	u := &User{}
	var isAdmin int
	var createdAt, updatedAt string
	if err := row.Scan(&u.ID, &u.Username, &u.PasswordHash, &u.DisplayName,
		&isAdmin, &createdAt, &updatedAt); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, ErrNotFound
		}
		return nil, err
	}
	u.IsAdmin = isAdmin != 0
	u.CreatedAt, _ = time.Parse(time.RFC3339, createdAt)
	u.UpdatedAt, _ = time.Parse(time.RFC3339, updatedAt)
	return u, nil
}
```

- [ ] **Step 4: 运行测试确认通过**

```bash
go test ./internal/user/
```

Expected: `ok  agentdocs/internal/user`。

- [ ] **Step 5: 提交**

```bash
gofmt -l .
git add internal/user/
git commit -m "feat: add user store"
```

### Task 7: 密码哈希 Argon2id（TDD）

**Files:**
- Create: `internal/auth/password.go`
- Test: `internal/auth/password_test.go`

- [ ] **Step 1: 写失败测试**

`internal/auth/password_test.go`：

```go
package auth

import "testing"

func TestHashAndVerifyPassword(t *testing.T) {
	const pw = "correct horse battery staple"
	hash, err := HashPassword(pw)
	if err != nil {
		t.Fatalf("HashPassword: %v", err)
	}
	ok, err := VerifyPassword(pw, hash)
	if err != nil || !ok {
		t.Fatalf("VerifyPassword: ok=%v err=%v", ok, err)
	}
	ok, err = VerifyPassword("wrong-password", hash)
	if err != nil {
		t.Fatalf("VerifyPassword wrong: %v", err)
	}
	if ok {
		t.Fatal("wrong password verified")
	}
}

func TestVerifyPasswordRejectsMalformedHash(t *testing.T) {
	if _, err := VerifyPassword("x", "not-a-hash"); err == nil {
		t.Fatal("want error for malformed hash")
	}
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
go test ./internal/auth/
```

Expected: FAIL，package 不存在。

- [ ] **Step 3: 实现密码哈希**

`internal/auth/password.go`：

```go
package auth

import (
	"crypto/rand"
	"crypto/subtle"
	"encoding/base64"
	"errors"
	"fmt"

	"golang.org/x/crypto/argon2"
)

const (
	argonTime    = 3
	argonMemory  = 64 * 1024
	argonThreads = 1
	argonKeyLen  = 32
)

// HashPassword returns an encoded Argon2id hash (PHC string format).
func HashPassword(password string) (string, error) {
	salt := make([]byte, 16)
	if _, err := rand.Read(salt); err != nil {
		return "", err
	}
	key := argon2.IDKey([]byte(password), salt, argonTime, argonMemory, argonThreads, argonKeyLen)
	return fmt.Sprintf("$argon2id$v=19$m=%d,t=%d,p=%d$%s$%s",
		argonMemory, argonTime, argonThreads,
		base64.RawStdEncoding.EncodeToString(salt),
		base64.RawStdEncoding.EncodeToString(key)), nil
}

// VerifyPassword checks a password against an encoded Argon2id hash.
func VerifyPassword(password, encoded string) (bool, error) {
	parts := splitHash(encoded)
	if parts == nil {
		return false, errors.New("invalid password hash format")
	}
	if _, err := parseVersion(parts[2]); err != nil {
		return false, err
	}
	m, t, p, err := parseParams(parts[3])
	if err != nil {
		return false, err
	}
	salt, err := base64.RawStdEncoding.DecodeString(parts[4])
	if err != nil {
		return false, err
	}
	expected, err := base64.RawStdEncoding.DecodeString(parts[5])
	if err != nil {
		return false, err
	}
	actual := argon2.IDKey([]byte(password), salt, t, m, p, uint32(len(expected)))
	return subtle.ConstantTimeCompare(actual, expected) == 1, nil
}

func splitHash(encoded string) []string {
	var parts []string
	start := 0
	for i := 0; i < len(encoded); i++ {
		if encoded[i] == '$' {
			parts = append(parts, encoded[start:i])
			start = i + 1
		}
	}
	parts = append(parts, encoded[start:])
	if len(parts) != 6 || parts[0] != "" || parts[1] != "argon2id" {
		return nil
	}
	return parts
}

func parseVersion(s string) (int, error) {
	var v int
	if _, err := fmt.Sscanf(s, "v=%d", &v); err != nil {
		return 0, err
	}
	if v != 19 {
		return 0, errors.New("unsupported argon2 version")
	}
	return v, nil
}

func parseParams(s string) (uint32, uint32, uint8, error) {
	var m, t, p int
	if _, err := fmt.Sscanf(s, "m=%d,t=%d,p=%d", &m, &t, &p); err != nil {
		return 0, 0, 0, err
	}
	return uint32(m), uint32(t), uint8(p), nil
}
```

- [ ] **Step 4: 运行测试确认通过**

```bash
go test ./internal/auth/
```

Expected: `ok  agentdocs/internal/auth`。

- [ ] **Step 5: 提交**

```bash
gofmt -l .
git add internal/auth/password.go internal/auth/password_test.go
git commit -m "feat: add argon2id password hashing"
```

### Task 8: 会话服务（TDD）

**Files:**
- Create: `internal/auth/service.go`
- Test: `internal/auth/service_test.go`

- [ ] **Step 1: 写失败测试**

`internal/auth/service_test.go`：

```go
package auth

import (
	"context"
	"testing"
	"time"

	"agentdocs/internal/platform/clock"
	"agentdocs/internal/store/sqlite"
	"agentdocs/internal/user"
)

type fakeClock struct{ now time.Time }

func (f *fakeClock) Now() time.Time { return f.now }

func newService(t *testing.T) (*Service, *user.Store, *fakeClock) {
	t.Helper()
	db, err := sqlite.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	clk := &fakeClock{now: time.Date(2026, 8, 2, 12, 0, 0, 0, time.UTC)}
	users := user.NewStore(db)
	return NewService(db, clk, time.Hour), users, clk
}

func createUser(t *testing.T, users *user.Store, id, username string) {
	t.Helper()
	now := time.Now().UTC()
	hash, err := HashPassword("secret123")
	if err != nil {
		t.Fatal(err)
	}
	u := &user.User{ID: id, Username: username, DisplayName: username,
		PasswordHash: hash, IsAdmin: true, CreatedAt: now, UpdatedAt: now}
	if err := users.Create(context.Background(), u); err != nil {
		t.Fatal(err)
	}
}

func TestLoginSuccess(t *testing.T) {
	svc, users, _ := newService(t)
	createUser(t, users, "usr_1", "admin")
	got, token, err := svc.Login(context.Background(), users, "admin", "secret123")
	if err != nil {
		t.Fatalf("Login: %v", err)
	}
	if got.ID != "usr_1" || token == "" {
		t.Fatalf("bad login result: %+v token=%q", got, token)
	}
}

func TestLoginWrongPassword(t *testing.T) {
	svc, users, _ := newService(t)
	createUser(t, users, "usr_1", "admin")
	if _, _, err := svc.Login(context.Background(), users, "admin", "wrong");
		err != ErrInvalidCredentials {
		t.Fatalf("want ErrInvalidCredentials, got %v", err)
	}
	if _, _, err := svc.Login(context.Background(), users, "nobody", "secret123");
		err != ErrInvalidCredentials {
		t.Fatalf("want ErrInvalidCredentials, got %v", err)
	}
}

func TestResolveSessionValidAndExpired(t *testing.T) {
	svc, users, clk := newService(t)
	createUser(t, users, "usr_1", "admin")
	token, err := svc.CreateSession(context.Background(), "usr_1")
	if err != nil {
		t.Fatal(err)
	}
	ses, u, err := svc.ResolveSession(context.Background(), token)
	if err != nil {
		t.Fatalf("resolve: %v", err)
	}
	if ses.UserID != "usr_1" || u.Username != "admin" {
		t.Fatalf("session mismatch: %+v %+v", ses, u)
	}

	// Advance past TTL: session must be rejected and removed.
	clk.now = clk.now.Add(2 * time.Hour)
	if _, _, err := svc.ResolveSession(context.Background(), token); err != ErrSessionNotFound {
		t.Fatalf("expired session: want ErrSessionNotFound, got %v", err)
	}
}

func TestResolveSessionUnknownToken(t *testing.T) {
	svc, _, _ := newService(t)
	if _, _, err := svc.ResolveSession(context.Background(), "garbage"); err != ErrSessionNotFound {
		t.Fatalf("want ErrSessionNotFound, got %v", err)
	}
}

func TestDeleteSessionByToken(t *testing.T) {
	svc, users, _ := newService(t)
	createUser(t, users, "usr_1", "admin")
	token, err := svc.CreateSession(context.Background(), "usr_1")
	if err != nil {
		t.Fatal(err)
	}
	if err := svc.DeleteSessionByToken(context.Background(), token); err != nil {
		t.Fatal(err)
	}
	if _, _, err := svc.ResolveSession(context.Background(), token); err != ErrSessionNotFound {
		t.Fatalf("want ErrSessionNotFound, got %v", err)
	}
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
go test ./internal/auth/
```

Expected: FAIL（`Service`、`ErrInvalidCredentials` 未定义）。

- [ ] **Step 3: 实现会话服务**

`internal/auth/service.go`：

```go
package auth

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"database/sql"
	"encoding/base64"
	"errors"
	"time"

	"agentdocs/internal/platform/clock"
	"agentdocs/internal/platform/id"
	"agentdocs/internal/user"
)

var (
	ErrInvalidCredentials = errors.New("invalid credentials")
	ErrSessionNotFound    = errors.New("session not found")
)

type Session struct {
	ID        string
	UserID    string
	TokenHash string
	ExpiresAt time.Time
	CreatedAt time.Time
}

// Service manages login and sessions. Only token hashes are stored.
type Service struct {
	db    *sql.DB
	clock clock.Clock
	ttl   time.Duration
}

func NewService(db *sql.DB, clk clock.Clock, ttl time.Duration) *Service {
	return &Service{db: db, clock: clk, ttl: ttl}
}

func hashToken(token string) string {
	sum := sha256.Sum256([]byte(token))
	return base64.RawStdEncoding.EncodeToString(sum[:])
}

// Login verifies credentials and creates a session, returning the user and
// the raw session token (shown to the client exactly once).
func (s *Service) Login(ctx context.Context, users *user.Store, username, password string) (*user.User, string, error) {
	u, err := users.GetByUsername(ctx, username)
	if err != nil {
		return nil, "", ErrInvalidCredentials
	}
	ok, err := VerifyPassword(password, u.PasswordHash)
	if err != nil || !ok {
		return nil, "", ErrInvalidCredentials
	}
	token, err := s.CreateSession(ctx, u.ID)
	if err != nil {
		return nil, "", err
	}
	return u, token, nil
}

func (s *Service) CreateSession(ctx context.Context, userID string) (string, error) {
	raw := make([]byte, 32)
	if _, err := rand.Read(raw); err != nil {
		return "", err
	}
	token := base64.RawURLEncoding.EncodeToString(raw)
	now := s.clock.Now().UTC()
	_, err := s.db.ExecContext(ctx, `
		INSERT INTO sessions (id, user_id, token_hash, expires_at, created_at, last_used_at)
		VALUES (?, ?, ?, ?, ?, ?)`,
		id.New("ses"), userID, hashToken(token),
		now.Add(s.ttl).Format(time.RFC3339),
		now.Format(time.RFC3339), now.Format(time.RFC3339))
	if err != nil {
		return "", err
	}
	return token, nil
}

// ResolveSession returns the session and its user for a raw token, or
// ErrSessionNotFound when the token is unknown or expired.
func (s *Service) ResolveSession(ctx context.Context, token string) (*Session, *user.User, error) {
	row := s.db.QueryRowContext(ctx, `
		SELECT s.id, s.user_id, s.expires_at, s.created_at,
		       u.id, u.username, u.display_name, u.is_admin, u.created_at, u.updated_at
		FROM sessions s
		JOIN users u ON u.id = s.user_id
		WHERE s.token_hash = ?`, hashToken(token))

	ses := &Session{}
	u := &user.User{}
	var isAdmin int
	var sesExpires, sesCreated, uCreated, uUpdated string
	err := row.Scan(&ses.ID, &ses.UserID, &sesExpires, &sesCreated,
		&u.ID, &u.Username, &u.DisplayName, &isAdmin, &uCreated, &uUpdated)
	if err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, nil, ErrSessionNotFound
		}
		return nil, nil, err
	}
	u.IsAdmin = isAdmin != 0
	ses.ExpiresAt, _ = time.Parse(time.RFC3339, sesExpires)
	ses.CreatedAt, _ = time.Parse(time.RFC3339, sesCreated)
	u.CreatedAt, _ = time.Parse(time.RFC3339, uCreated)
	u.UpdatedAt, _ = time.Parse(time.RFC3339, uUpdated)

	if !ses.ExpiresAt.After(s.clock.Now()) {
		_ = s.DeleteSession(ctx, ses.ID)
		return nil, nil, ErrSessionNotFound
	}
	_, _ = s.db.ExecContext(ctx, `UPDATE sessions SET last_used_at = ? WHERE id = ?`,
		s.clock.Now().UTC().Format(time.RFC3339), ses.ID)
	return ses, u, nil
}

func (s *Service) DeleteSession(ctx context.Context, sessionID string) error {
	_, err := s.db.ExecContext(ctx, `DELETE FROM sessions WHERE id = ?`, sessionID)
	return err
}

func (s *Service) DeleteSessionByToken(ctx context.Context, token string) error {
	_, err := s.db.ExecContext(ctx, `DELETE FROM sessions WHERE token_hash = ?`, hashToken(token))
	return err
}
```

- [ ] **Step 4: 运行测试确认通过**

```bash
go test ./internal/auth/
```

Expected: `ok  agentdocs/internal/auth`。

- [ ] **Step 5: 提交**

```bash
gofmt -l .
git add internal/auth/service.go internal/auth/service_test.go
git commit -m "feat: add session service"
```

### Task 9: HTTP 请求/响应辅助（TDD）

**Files:**
- Create: `internal/httpapi/request/request.go`、`internal/httpapi/response/response.go`
- Test: `internal/httpapi/request/request_test.go`、`internal/httpapi/response/response_test.go`

- [ ] **Step 1: 写失败测试**

`internal/httpapi/request/request_test.go`：

```go
package request

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestDecodeJSON(t *testing.T) {
	req := httptest.NewRequest(http.MethodPost, "/",
		strings.NewReader(`{"username":"admin"}`))
	rec := httptest.NewRecorder()
	var v struct {
		Username string `json:"username"`
	}
	if err := DecodeJSON(rec, req, &v, 1024); err != nil {
		t.Fatalf("DecodeJSON: %v", err)
	}
	if v.Username != "admin" {
		t.Fatalf("username = %q", v.Username)
	}
}

func TestDecodeJSONRejectsBadJSON(t *testing.T) {
	req := httptest.NewRequest(http.MethodPost, "/",
		strings.NewReader(`{"username":`))
	rec := httptest.NewRecorder()
	var v map[string]any
	if err := DecodeJSON(rec, req, &v, 1024); err == nil {
		t.Fatal("want error for malformed JSON")
	}
}

func TestDecodeJSONRejectsOversizedBody(t *testing.T) {
	req := httptest.NewRequest(http.MethodPost, "/",
		strings.NewReader(strings.Repeat("x", 2048)))
	rec := httptest.NewRecorder()
	var v map[string]any
	if err := DecodeJSON(rec, req, &v, 100); err == nil {
		t.Fatal("want error for oversized body")
	}
}

func TestRequestIDContext(t *testing.T) {
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	ctx := WithRequestID(req.Context(), "req_123")
	if got := RequestID(req.WithContext(ctx)); got != "req_123" {
		t.Fatalf("RequestID = %q", got)
	}
	if got := RequestID(req); got != "" {
		t.Fatalf("RequestID without ctx = %q", got)
	}
}
```

`internal/httpapi/response/response_test.go`：

```go
package response

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"agentdocs/internal/httpapi/request"
)

func TestWriteErrorEnvelope(t *testing.T) {
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	req = req.WithContext(request.WithRequestID(req.Context(), "req_123"))
	rec := httptest.NewRecorder()
	WriteError(rec, req, http.StatusConflict, "revision_conflict", "Project revision has changed.")
	if rec.Code != http.StatusConflict {
		t.Fatalf("status = %d", rec.Code)
	}
	var body struct {
		Error struct {
			Code      string `json:"code"`
			Message   string `json:"message"`
			RequestID string `json:"request_id"`
		} `json:"error"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	if body.Error.Code != "revision_conflict" || body.Error.RequestID != "req_123" {
		t.Fatalf("bad envelope: %+v", body)
	}
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
go test ./internal/httpapi/...
```

Expected: FAIL，package 不存在。

- [ ] **Step 3: 实现 request 包**

`internal/httpapi/request/request.go`：

```go
package request

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
)

type ctxKey int

const requestIDKey ctxKey = 0

func WithRequestID(ctx context.Context, id string) context.Context {
	return context.WithValue(ctx, requestIDKey, id)
}

func RequestID(r *http.Request) string {
	id, _ := r.Context().Value(requestIDKey).(string)
	return id
}

// DecodeJSON decodes a request body with a size limit and strict fields.
func DecodeJSON(w http.ResponseWriter, r *http.Request, dst any, maxBytes int64) error {
	r.Body = http.MaxBytesReader(w, r.Body, maxBytes)
	dec := json.NewDecoder(r.Body)
	dec.DisallowUnknownFields()
	if err := dec.Decode(dst); err != nil {
		var maxErr *http.MaxBytesError
		if errors.As(err, &maxErr) {
			return fmt.Errorf("request body exceeds %d bytes", maxBytes)
		}
		return fmt.Errorf("invalid JSON body")
	}
	return nil
}
```

- [ ] **Step 4: 实现 response 包**

`internal/httpapi/response/response.go`：

```go
package response

import (
	"encoding/json"
	"net/http"

	"agentdocs/internal/httpapi/request"
)

// ErrorBody is the unified error envelope (spec §20).
type ErrorBody struct {
	Error ErrorDetails `json:"error"`
}

type ErrorDetails struct {
	Code      string `json:"code"`
	Message   string `json:"message"`
	RequestID string `json:"request_id"`
}

func WriteJSON(w http.ResponseWriter, status int, v any) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(v)
}

func WriteError(w http.ResponseWriter, r *http.Request, status int, code, message string) {
	WriteJSON(w, status, ErrorBody{
		Error: ErrorDetails{
			Code:      code,
			Message:   message,
			RequestID: request.RequestID(r),
		},
	})
}
```

- [ ] **Step 5: 运行测试确认通过**

```bash
go test ./internal/httpapi/...
```

Expected: `ok  agentdocs/internal/httpapi/request`、`ok  agentdocs/internal/httpapi/response`。

- [ ] **Step 6: 提交**

```bash
gofmt -l .
git add internal/httpapi/request internal/httpapi/response
git commit -m "feat: add http request/response helpers"
```

### Task 10: HTTP 中间件（TDD）

**Files:**
- Create: `internal/httpapi/middleware/logging.go`、`internal/httpapi/middleware/session.go`
- Test: `internal/httpapi/middleware/middleware_test.go`

- [ ] **Step 1: 写失败测试**

`internal/httpapi/middleware/middleware_test.go`：

```go
package middleware

import (
	"context"
	"encoding/json"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"agentdocs/internal/auth"
	"agentdocs/internal/httpapi/request"
	"agentdocs/internal/platform/clock"
	"agentdocs/internal/store/sqlite"
	"agentdocs/internal/user"
)

func newAuthService(t *testing.T) *auth.Service {
	t.Helper()
	db, err := sqlite.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	return auth.NewService(db, clock.Real{}, 24*time.Hour)
}

func discardLog() *slog.Logger {
	return slog.New(slog.NewTextHandler(io.Discard, nil))
}

func TestRequestIDMiddleware(t *testing.T) {
	var got string
	h := RequestID(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		got = request.RequestID(r)
		w.WriteHeader(http.StatusOK)
	}))
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/", nil))
	if got == "" {
		t.Fatal("request id not set")
	}
	if rec.Header().Get("X-Request-ID") != got {
		t.Fatal("request id header mismatch")
	}
}

func TestSessionAuthRejectsMissingOrInvalidCookie(t *testing.T) {
	h := SessionAuth(newAuthService(t))(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/", nil))
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("no cookie: status = %d, want 401", rec.Code)
	}

	rec = httptest.NewRecorder()
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	req.AddCookie(&http.Cookie{Name: "agentdocs_session", Value: "garbage"})
	h.ServeHTTP(rec, req)
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("bad cookie: status = %d, want 401", rec.Code)
	}

	var body struct {
		Error struct {
			Code string `json:"code"`
		} `json:"error"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	if body.Error.Code != "authentication_required" {
		t.Fatalf("code = %q", body.Error.Code)
	}
}

func TestSessionAuthAcceptsValidSession(t *testing.T) {
	db, err := sqlite.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	users := user.NewStore(db)
	now := time.Now().UTC()
	if err := users.Create(context.Background(), &user.User{
		ID: "usr_1", Username: "admin", DisplayName: "Admin",
		PasswordHash: "x", IsAdmin: true, CreatedAt: now, UpdatedAt: now,
	}); err != nil {
		t.Fatal(err)
	}
	svc := auth.NewService(db, clock.Real{}, 24*time.Hour)
	token, err := svc.CreateSession(context.Background(), "usr_1")
	if err != nil {
		t.Fatal(err)
	}

	h := SessionAuth(svc)(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		u := UserFrom(r)
		if u == nil || u.Username != "admin" {
			t.Errorf("user not in context: %+v", u)
		}
		w.WriteHeader(http.StatusOK)
	}))
	rec := httptest.NewRecorder()
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	req.AddCookie(&http.Cookie{Name: "agentdocs_session", Value: token})
	h.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200", rec.Code)
	}
}

func TestRecoverer(t *testing.T) {
	h := Recoverer(discardLog())(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		panic("boom")
	}))
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/", nil))
	if rec.Code != http.StatusInternalServerError {
		t.Fatalf("status = %d, want 500", rec.Code)
	}
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
go test ./internal/httpapi/middleware/
```

Expected: FAIL，package 不存在。

- [ ] **Step 3: 实现 logging 中间件**

`internal/httpapi/middleware/logging.go`：

```go
package middleware

import (
	"log/slog"
	"net/http"
	"time"

	"github.com/oklog/ulid/v2"

	"agentdocs/internal/httpapi/request"
	"agentdocs/internal/httpapi/response"
)

// RequestID ensures every request has an ID and echoes it in the response.
func RequestID(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		id := r.Header.Get("X-Request-ID")
		if id == "" {
			id = "req_" + ulid.Make().String()
		}
		w.Header().Set("X-Request-ID", id)
		next.ServeHTTP(w, r.WithContext(request.WithRequestID(r.Context(), id)))
	})
}

type statusWriter struct {
	http.ResponseWriter
	status int
}

func (w *statusWriter) WriteHeader(code int) {
	w.status = code
	w.ResponseWriter.WriteHeader(code)
}

// RequestLogger emits one structured log line per request.
func RequestLogger(log *slog.Logger) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			start := time.Now()
			rw := &statusWriter{ResponseWriter: w, status: http.StatusOK}
			next.ServeHTTP(rw, r)
			log.Info("http_request",
				"request_id", request.RequestID(r),
				"method", r.Method,
				"path", r.URL.Path,
				"status", rw.status,
				"duration_ms", time.Since(start).Milliseconds(),
			)
		})
	}
}

// Recoverer converts panics into a JSON 500 and keeps the server alive.
func Recoverer(log *slog.Logger) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			defer func() {
				if rec := recover(); rec != nil {
					log.Error("panic", "request_id", request.RequestID(r), "panic", rec)
					response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "internal error")
				}
			}()
			next.ServeHTTP(w, r)
		})
	}
}
```

- [ ] **Step 4: 实现 session 中间件**

`internal/httpapi/middleware/session.go`：

```go
package middleware

import (
	"context"
	"net/http"

	"agentdocs/internal/auth"
	"agentdocs/internal/httpapi/response"
	"agentdocs/internal/user"
)

type ctxKey int

const userKey ctxKey = 0

// SessionAuth requires a valid session cookie and stores the user in context.
func SessionAuth(svc *auth.Service) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			cookie, err := r.Cookie("agentdocs_session")
			if err != nil {
				response.WriteError(w, r, http.StatusUnauthorized, "authentication_required", "login required")
				return
			}
			_, u, err := svc.ResolveSession(r.Context(), cookie.Value)
			if err != nil {
				response.WriteError(w, r, http.StatusUnauthorized, "authentication_required", "session is invalid or expired")
				return
			}
			ctx := context.WithValue(r.Context(), userKey, u)
			next.ServeHTTP(w, r.WithContext(ctx))
		})
	}
}

// UserFrom returns the authenticated user stored by SessionAuth, or nil.
func UserFrom(r *http.Request) *user.User {
	u, _ := r.Context().Value(userKey).(*user.User)
	return u
}
```

- [ ] **Step 5: 运行测试确认通过**

```bash
go test ./internal/httpapi/middleware/
```

Expected: `ok  agentdocs/internal/httpapi/middleware`。

- [ ] **Step 6: 提交**

```bash
gofmt -l .
git add internal/httpapi/middleware/
git commit -m "feat: add http middleware (request id, logging, recovery, session auth)"
```

### Task 11: 认证处理器 + 路由 + 静态资源嵌入（TDD）

**Files:**
- Create: `internal/httpapi/handlers/auth.go`、`internal/server/router.go`
- Create: `web/embed.go`、`web/dist/index.html`（占位符）
- Test: `internal/server/router_test.go`

- [ ] **Step 1: 创建前端占位符与 embed**

`web/dist/index.html`：

```html
<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8" />
    <title>AgentDocs</title>
  </head>
  <body>
    <p>AgentDocs placeholder — run `npm run build` in web/ to generate the real bundle.</p>
  </body>
</html>
```

`web/embed.go`：

```go
// Package web embeds the built frontend so the Go binary is self-contained.
package web

import "embed"

// Dist contains the built frontend (web/dist). The committed placeholder
// index.html keeps go:embed working on fresh checkouts; `npm run build`
// replaces it with the real bundle.
//
//go:embed dist
var Dist embed.FS
```

- [ ] **Step 2: 写失败测试**

`internal/server/router_test.go`：

```go
package server

import (
	"encoding/json"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"agentdocs/internal/auth"
	"agentdocs/internal/config"
	"agentdocs/internal/platform/clock"
	"agentdocs/internal/store/sqlite"
	"agentdocs/internal/user"
)

func newTestRouter(t *testing.T) http.Handler {
	t.Helper()
	cfg := config.Load()
	cfg.DataDir = t.TempDir()
	db, err := sqlite.Open(cfg.DataDir)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	users := user.NewStore(db)
	authSvc := auth.NewService(db, clock.Real{}, 24*time.Hour)
	log := slog.New(slog.NewTextHandler(io.Discard, nil))
	return NewRouter(cfg, log, db, users, authSvc)
}

func TestHealthAndReady(t *testing.T) {
	h := newTestRouter(t)
	for _, path := range []string{"/healthz", "/readyz"} {
		rec := httptest.NewRecorder()
		h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, path, nil))
		if rec.Code != http.StatusOK {
			t.Fatalf("%s: status = %d", path, rec.Code)
		}
	}
}

func TestUnknownAPIPathReturnsJSON404(t *testing.T) {
	h := newTestRouter(t)
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/api/v1/nope", nil))
	if rec.Code != http.StatusNotFound {
		t.Fatalf("status = %d, want 404", rec.Code)
	}
	var body struct {
		Error struct {
			Code string `json:"code"`
		} `json:"error"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	if body.Error.Code != "not_found" {
		t.Fatalf("code = %q", body.Error.Code)
	}
}

func TestSPAServesPlaceholderAndFallsBack(t *testing.T) {
	h := newTestRouter(t)
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/", nil))
	if rec.Code != http.StatusOK || !strings.Contains(rec.Body.String(), "AgentDocs placeholder") {
		t.Fatalf("root: status=%d body=%q", rec.Code, rec.Body.String())
	}
	rec = httptest.NewRecorder()
	h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/login", nil))
	if rec.Code != http.StatusOK || !strings.Contains(rec.Body.String(), "AgentDocs placeholder") {
		t.Fatalf("spa fallback: status=%d body=%q", rec.Code, rec.Body.String())
	}
}
```

- [ ] **Step 3: 运行测试确认失败**

```bash
go test ./internal/server/
```

Expected: FAIL（`NewRouter` 未定义、`web` 包不存在）。

- [ ] **Step 4: 实现认证处理器**

`internal/httpapi/handlers/auth.go`：

```go
package handlers

import (
	"log/slog"
	"net/http"

	"agentdocs/internal/auth"
	"agentdocs/internal/config"
	"agentdocs/internal/httpapi/middleware"
	"agentdocs/internal/httpapi/request"
	"agentdocs/internal/httpapi/response"
	"agentdocs/internal/user"
)

type AuthHandler struct {
	cfg   *config.Config
	svc   *auth.Service
	users *user.Store
	log   *slog.Logger
}

func NewAuthHandler(cfg *config.Config, svc *auth.Service, users *user.Store, log *slog.Logger) *AuthHandler {
	return &AuthHandler{cfg: cfg, svc: svc, users: users, log: log}
}

type loginRequest struct {
	Username string `json:"username"`
	Password string `json:"password"`
}

func (h *AuthHandler) Login(w http.ResponseWriter, r *http.Request) {
	var req loginRequest
	if err := request.DecodeJSON(w, r, &req, h.cfg.MaxBodyBytes); err != nil {
		response.WriteError(w, r, http.StatusBadRequest, "validation_failed", "invalid request body")
		return
	}
	u, token, err := h.svc.Login(r.Context(), h.users, req.Username, req.Password)
	if err != nil {
		response.WriteError(w, r, http.StatusUnauthorized, "invalid_credentials", "invalid username or password")
		return
	}
	http.SetCookie(w, &http.Cookie{
		Name:     "agentdocs_session",
		Value:    token,
		Path:     "/",
		HttpOnly: true,
		Secure:   h.cfg.SecureCookies,
		SameSite: http.SameSiteLaxMode,
		MaxAge:   int(h.cfg.SessionTTL.Seconds()),
	})
	response.WriteJSON(w, http.StatusOK, map[string]any{"user": publicUser(u)})
}

func (h *AuthHandler) Logout(w http.ResponseWriter, r *http.Request) {
	if cookie, err := r.Cookie("agentdocs_session"); err == nil {
		_ = h.svc.DeleteSessionByToken(r.Context(), cookie.Value)
	}
	http.SetCookie(w, &http.Cookie{
		Name:     "agentdocs_session",
		Value:    "",
		Path:     "/",
		HttpOnly: true,
		Secure:   h.cfg.SecureCookies,
		SameSite: http.SameSiteLaxMode,
		MaxAge:   -1,
	})
	response.WriteJSON(w, http.StatusOK, map[string]any{"ok": true})
}

func (h *AuthHandler) Me(w http.ResponseWriter, r *http.Request) {
	u := middleware.UserFrom(r)
	response.WriteJSON(w, http.StatusOK, map[string]any{"user": publicUser(u)})
}

type passwordRequest struct {
	CurrentPassword string `json:"current_password"`
	NewPassword     string `json:"new_password"`
}

func (h *AuthHandler) Password(w http.ResponseWriter, r *http.Request) {
	var req passwordRequest
	if err := request.DecodeJSON(w, r, &req, h.cfg.MaxBodyBytes); err != nil {
		response.WriteError(w, r, http.StatusBadRequest, "validation_failed", "invalid request body")
		return
	}
	if len(req.NewPassword) < 8 {
		response.WriteError(w, r, http.StatusBadRequest, "validation_failed", "new password must be at least 8 characters")
		return
	}
	me := middleware.UserFrom(r)
	fresh, err := h.users.GetByUsername(r.Context(), me.Username)
	if err != nil {
		response.WriteError(w, r, http.StatusUnauthorized, "invalid_credentials", "current password is incorrect")
		return
	}
	ok, err := auth.VerifyPassword(req.CurrentPassword, fresh.PasswordHash)
	if err != nil || !ok {
		response.WriteError(w, r, http.StatusUnauthorized, "invalid_credentials", "current password is incorrect")
		return
	}
	hash, err := auth.HashPassword(req.NewPassword)
	if err != nil {
		h.log.Error("hash password", "error", err)
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "internal error")
		return
	}
	if err := h.users.UpdatePassword(r.Context(), fresh.ID, hash); err != nil {
		h.log.Error("update password", "error", err)
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "internal error")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"ok": true})
}

func publicUser(u *user.User) map[string]any {
	return map[string]any{
		"id":           u.ID,
		"username":     u.Username,
		"display_name": u.DisplayName,
		"is_admin":     u.IsAdmin,
	}
}
```

- [ ] **Step 5: 实现路由**

`internal/server/router.go`：

```go
package server

import (
	"database/sql"
	"io/fs"
	"log/slog"
	"net/http"
	"strings"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/cors"

	"agentdocs/internal/auth"
	"agentdocs/internal/config"
	"agentdocs/internal/httpapi/handlers"
	"agentdocs/internal/httpapi/middleware"
	"agentdocs/internal/httpapi/response"
	"agentdocs/internal/user"
	"agentdocs/web"
)

func NewRouter(cfg *config.Config, log *slog.Logger, db *sql.DB, users *user.Store, authSvc *auth.Service) http.Handler {
	r := chi.NewRouter()

	r.Use(middleware.RequestID)
	r.Use(middleware.RequestLogger(log))
	r.Use(middleware.Recoverer(log))
	r.Use(cors.Handler(cors.Options{
		AllowedOrigins:   []string{cfg.WebOrigin},
		AllowedMethods:   []string{"GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"},
		AllowedHeaders:   []string{"Content-Type", "Authorization", "Idempotency-Key", "X-Request-ID"},
		AllowCredentials: true,
		MaxAge:           300,
	}))

	h := handlers.NewAuthHandler(cfg, authSvc, users, log)

	r.Get("/healthz", func(w http.ResponseWriter, r *http.Request) {
		response.WriteJSON(w, http.StatusOK, map[string]any{"status": "ok"})
	})
	r.Get("/readyz", func(w http.ResponseWriter, r *http.Request) {
		if err := db.PingContext(r.Context()); err != nil {
			response.WriteError(w, r, http.StatusServiceUnavailable, "not_ready", "database unavailable")
			return
		}
		response.WriteJSON(w, http.StatusOK, map[string]any{"status": "ready"})
	})

	r.Route("/api/v1", func(r chi.Router) {
		r.Route("/auth", func(r chi.Router) {
			r.Post("/login", h.Login)
			r.Post("/logout", h.Logout)
			r.Group(func(r chi.Router) {
				r.Use(middleware.SessionAuth(authSvc))
				r.Get("/me", h.Me)
				r.Post("/password", h.Password)
			})
		})
	})

	r.Handle("/*", spaHandler())
	return r
}

// spaHandler serves the embedded frontend with SPA fallback; unknown /api
// paths get a JSON 404 instead of the SPA index.
func spaHandler() http.Handler {
	sub, err := fs.Sub(web.Dist, "dist")
	if err != nil {
		panic(err)
	}
	files := http.FileServer(http.FS(sub))
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if strings.HasPrefix(r.URL.Path, "/api/") {
			response.WriteError(w, r, http.StatusNotFound, "not_found", "resource not found")
			return
		}
		path := strings.TrimPrefix(r.URL.Path, "/")
		if path == "" {
			path = "index.html"
		}
		if _, err := fs.Stat(sub, path); err != nil {
			r.URL.Path = "/"
		}
		files.ServeHTTP(w, r)
	})
}
```

- [ ] **Step 6: 运行测试确认通过**

```bash
go test ./internal/server/
```

Expected: `ok  agentdocs/internal/server`。

- [ ] **Step 7: 提交**

```bash
gofmt -l .
git add internal/httpapi/handlers internal/server web/embed.go web/dist
git commit -m "feat: add auth handlers, router and embedded static assets"
```

### Task 12: 应用装配 + 管理员创建（TDD）

**Files:**
- Create: `internal/app/app.go`
- Test: `internal/app/app_test.go`

- [ ] **Step 1: 写失败测试**

`internal/app/app_test.go`：

```go
package app

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"agentdocs/internal/config"
)

func newTestApp(t *testing.T) *App {
	t.Helper()
	cfg := config.Load()
	cfg.DataDir = t.TempDir()
	a, err := New(cfg)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { a.Close() })
	return a
}

func createAdmin(t *testing.T, a *App) {
	t.Helper()
	if err := a.CreateAdmin(context.Background(), "admin", "secret123"); err != nil {
		t.Fatal(err)
	}
}

func doLogin(t *testing.T, h http.Handler, username, password string) (int, []*http.Cookie, string) {
	t.Helper()
	body := strings.NewReader(`{"username":"` + username + `","password":"` + password + `"}`)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/login", body)
	req.Header.Set("Content-Type", "application/json")
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, req)
	return rec.Code, rec.Result().Cookies(), rec.Body.String()
}

func TestCreateAdmin(t *testing.T) {
	a := newTestApp(t)
	if err := a.CreateAdmin(context.Background(), "admin", "secret123"); err != nil {
		t.Fatalf("CreateAdmin: %v", err)
	}
	if err := a.CreateAdmin(context.Background(), "admin", "secret123"); err == nil {
		t.Fatal("duplicate admin allowed")
	}
	if err := a.CreateAdmin(context.Background(), "ab", "secret123"); err == nil {
		t.Fatal("short username allowed")
	}
	if err := a.CreateAdmin(context.Background(), "admin2", "short"); err == nil {
		t.Fatal("short password allowed")
	}
}

func TestLoginWrongPasswordReturns401(t *testing.T) {
	a := newTestApp(t)
	createAdmin(t, a)
	code, _, body := doLogin(t, a.Handler(), "admin", "wrong")
	if code != http.StatusUnauthorized {
		t.Fatalf("status = %d, body=%s", code, body)
	}
}

func TestLoginSuccessAndMe(t *testing.T) {
	a := newTestApp(t)
	createAdmin(t, a)
	code, cookies, body := doLogin(t, a.Handler(), "admin", "secret123")
	if code != http.StatusOK {
		t.Fatalf("login status = %d, body=%s", code, body)
	}
	if len(cookies) == 0 {
		t.Fatal("no session cookie")
	}
	for _, c := range cookies {
		if c.Name == "agentdocs_session" {
			if !c.HttpOnly {
				t.Fatal("session cookie not HttpOnly")
			}
		}
	}
	req := httptest.NewRequest(http.MethodGet, "/api/v1/auth/me", nil)
	for _, c := range cookies {
		req.AddCookie(c)
	}
	rec := httptest.NewRecorder()
	a.Handler().ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("me status = %d, body=%s", rec.Code, rec.Body.String())
	}
	var resp struct {
		User struct {
			Username string `json:"username"`
			IsAdmin  bool   `json:"is_admin"`
		} `json:"user"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &resp); err != nil {
		t.Fatal(err)
	}
	if resp.User.Username != "admin" || !resp.User.IsAdmin {
		t.Fatalf("unexpected me body: %s", rec.Body.String())
	}
}

func TestMeRequiresSession(t *testing.T) {
	a := newTestApp(t)
	createAdmin(t, a)
	rec := httptest.NewRecorder()
	a.Handler().ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/api/v1/auth/me", nil))
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("status = %d, want 401", rec.Code)
	}
}

func TestLogout(t *testing.T) {
	a := newTestApp(t)
	createAdmin(t, a)
	_, cookies, _ := doLogin(t, a.Handler(), "admin", "secret123")

	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/logout", nil)
	for _, c := range cookies {
		req.AddCookie(c)
	}
	rec := httptest.NewRecorder()
	a.Handler().ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("logout status = %d", rec.Code)
	}

	req = httptest.NewRequest(http.MethodGet, "/api/v1/auth/me", nil)
	for _, c := range cookies {
		req.AddCookie(c)
	}
	rec = httptest.NewRecorder()
	a.Handler().ServeHTTP(rec, req)
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("me after logout status = %d, want 401", rec.Code)
	}
}

func TestChangePassword(t *testing.T) {
	a := newTestApp(t)
	createAdmin(t, a)
	_, cookies, _ := doLogin(t, a.Handler(), "admin", "secret123")

	post := func(payload string) *httptest.ResponseRecorder {
		req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/password",
			strings.NewReader(payload))
		req.Header.Set("Content-Type", "application/json")
		for _, c := range cookies {
			req.AddCookie(c)
		}
		rec := httptest.NewRecorder()
		a.Handler().ServeHTTP(rec, req)
		return rec
	}

	if rec := post(`{"current_password":"wrong","new_password":"newsecret456"}`); rec.Code != http.StatusUnauthorized {
		t.Fatalf("wrong current password: status = %d", rec.Code)
	}
	if rec := post(`{"current_password":"secret123","new_password":"newsecret456"}`); rec.Code != http.StatusOK {
		t.Fatalf("change password: status = %d", rec.Code)
	}
	if code, _, _ := doLogin(t, a.Handler(), "admin", "secret123"); code != http.StatusUnauthorized {
		t.Fatalf("old password still works: status = %d", code)
	}
	if code, _, _ := doLogin(t, a.Handler(), "admin", "newsecret456"); code != http.StatusOK {
		t.Fatalf("new password rejected: status = %d", code)
	}
}

func TestSessionPersistsAcrossRestart(t *testing.T) {
	cfg := config.Load()
	cfg.DataDir = t.TempDir()

	a1, err := New(cfg)
	if err != nil {
		t.Fatal(err)
	}
	createAdmin(t, a1)
	_, cookies, _ := doLogin(t, a1.Handler(), "admin", "secret123")
	if err := a1.Close(); err != nil {
		t.Fatal(err)
	}

	// Simulate a server restart: new App over the same data directory.
	a2, err := New(cfg)
	if err != nil {
		t.Fatal(err)
	}
	defer a2.Close()
	req := httptest.NewRequest(http.MethodGet, "/api/v1/auth/me", nil)
	for _, c := range cookies {
		req.AddCookie(c)
	}
	rec := httptest.NewRecorder()
	a2.Handler().ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("me after restart = %d, want 200", rec.Code)
	}
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
go test ./internal/app/
```

Expected: FAIL，package 不存在。

- [ ] **Step 3: 实现 app 装配**

`internal/app/app.go`：

```go
package app

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"log/slog"
	"net/http"
	"strings"
	"time"

	"agentdocs/internal/auth"
	"agentdocs/internal/config"
	"agentdocs/internal/platform/clock"
	"agentdocs/internal/platform/id"
	"agentdocs/internal/server"
	"agentdocs/internal/store/sqlite"
	"agentdocs/internal/user"
)

// App wires configuration, storage, services and the HTTP handler.
type App struct {
	cfg     *config.Config
	log     *slog.Logger
	db      *sql.DB
	clock   clock.Clock
	users   *user.Store
	authSvc *auth.Service
	handler http.Handler
}

func New(cfg *config.Config) (*App, error) {
	log := slog.Default()
	db, err := sqlite.Open(cfg.DataDir)
	if err != nil {
		return nil, fmt.Errorf("open sqlite: %w", err)
	}
	clk := clock.Real{}
	users := user.NewStore(db)
	authSvc := auth.NewService(db, clk, cfg.SessionTTL)
	handler := server.NewRouter(cfg, log, db, users, authSvc)
	return &App{
		cfg: cfg, log: log, db: db, clock: clk,
		users: users, authSvc: authSvc, handler: handler,
	}, nil
}

func (a *App) Handler() http.Handler { return a.handler }

func (a *App) Close() error { return a.db.Close() }

// CreateAdmin creates the first administrator user (idempotency: duplicate
// username is an error).
func (a *App) CreateAdmin(ctx context.Context, username, password string) error {
	username = strings.TrimSpace(username)
	if len(username) < 3 {
		return errors.New("username must be at least 3 characters")
	}
	if len(password) < 8 {
		return errors.New("password must be at least 8 characters")
	}
	if _, err := a.users.GetByUsername(ctx, username); err == nil {
		return fmt.Errorf("user %q already exists", username)
	} else if !errors.Is(err, user.ErrNotFound) {
		return err
	}
	hash, err := auth.HashPassword(password)
	if err != nil {
		return err
	}
	now := a.clock.Now().UTC()
	u := &user.User{
		ID: id.New("usr"), Username: username, DisplayName: username,
		PasswordHash: hash, IsAdmin: true, CreatedAt: now, UpdatedAt: now,
	}
	return a.users.Create(ctx, u)
}

// Run serves HTTP until ctx is cancelled, then shuts down gracefully.
func (a *App) Run(ctx context.Context) error {
	srv := &http.Server{
		Addr:              a.cfg.HTTPAddr,
		Handler:           a.handler,
		ReadHeaderTimeout: 5 * time.Second,
	}
	errCh := make(chan error, 1)
	go func() { errCh <- srv.ListenAndServe() }()
	a.log.Info("server started", "addr", a.cfg.HTTPAddr)
	select {
	case err := <-errCh:
		return err
	case <-ctx.Done():
		shCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		return srv.Shutdown(shCtx)
	}
}
```

- [ ] **Step 4: 运行测试确认通过**

```bash
go test ./internal/app/ -v
```

Expected: 全部 PASS，包含 `TestSessionPersistsAcrossRestart`（验收点 3）。

- [ ] **Step 5: 提交**

```bash
gofmt -l .
git add internal/app/
git commit -m "feat: wire application and admin creation"
```

### Task 13: CLI（serve / admin create）

**Files:**
- Create: `cmd/agentdocs/main.go`

- [ ] **Step 1: 实现 CLI**

`cmd/agentdocs/main.go`：

```go
package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"os/signal"
	"syscall"

	"agentdocs/internal/app"
	"agentdocs/internal/config"
)

func main() {
	slog.SetDefault(slog.New(slog.NewJSONHandler(os.Stderr, nil)))
	if err := run(os.Args[1:]); err != nil {
		slog.Error("command failed", "error", err)
		os.Exit(1)
	}
}

func run(args []string) error {
	if len(args) == 0 {
		return usageError()
	}
	switch args[0] {
	case "serve":
		return serve(args[1:])
	case "admin":
		return admin(args[1:])
	case "help", "-h", "--help":
		fmt.Fprint(os.Stdout, usageText)
		return nil
	default:
		return fmt.Errorf("unknown command %q", args[0])
	}
}

func serve(args []string) error {
	fs := flag.NewFlagSet("serve", flag.ExitOnError)
	dataDir := fs.String("data-dir", "", "data directory (default: $AGENTDOCS_DATA_DIR or data)")
	httpAddr := fs.String("http-addr", "", "HTTP listen address (default: $AGENTDOCS_HTTP_ADDR or :8080)")
	_ = fs.Parse(args)

	cfg := config.Load()
	if *dataDir != "" {
		cfg.DataDir = *dataDir
	}
	if *httpAddr != "" {
		cfg.HTTPAddr = *httpAddr
	}

	a, err := app.New(cfg)
	if err != nil {
		return err
	}
	defer a.Close()

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	return a.Run(ctx)
}

func admin(args []string) error {
	if len(args) == 0 || args[0] != "create" {
		return errors.New("usage: agentdocs admin create -username <name> [-password <pw>]")
	}
	fs := flag.NewFlagSet("admin create", flag.ExitOnError)
	username := fs.String("username", "", "admin username")
	password := fs.String("password", "", "admin password (fallback: $AGENTDOCS_ADMIN_PASSWORD)")
	_ = fs.Parse(args[1:])
	if *username == "" {
		return errors.New("username is required")
	}
	pw := *password
	if pw == "" {
		pw = os.Getenv("AGENTDOCS_ADMIN_PASSWORD")
	}
	if pw == "" {
		return errors.New("password is required (flag -password or env AGENTDOCS_ADMIN_PASSWORD)")
	}
	a, err := app.New(config.Load())
	if err != nil {
		return err
	}
	defer a.Close()
	return a.CreateAdmin(context.Background(), *username, pw)
}

func usageError() error {
	fmt.Fprint(os.Stdout, usageText)
	return errors.New("missing command")
}

const usageText = `AgentDocs - Git-backed documentation server for humans and AI agents

Usage:
  agentdocs serve              start the HTTP server
  agentdocs admin create       create the first administrator user
  agentdocs help               show this help
`
```

- [ ] **Step 2: 编译**

```bash
go build -o agentdocs ./cmd/agentdocs
```

Expected: 成功生成 `agentdocs` 二进制（`agentdocs` 已被 .gitignore 忽略）。

- [ ] **Step 3: CLI 冒烟测试**

```bash
./agentdocs help
./agentdocs admin create -username admin -password secret123
```

Expected: 第一条输出 usage；第二条无输出（成功）。再执行一次第二条：

```bash
./agentdocs admin create -username admin -password secret123
```

Expected: 报错 `user "admin" already exists`，退出码非 0。

- [ ] **Step 4: 服务冒烟测试（含重启验证）**

```bash
./agentdocs serve > /tmp/agentdocs.log 2>&1 &
sleep 1
curl -s http://127.0.0.1:8080/healthz
curl -s http://127.0.0.1:8080/readyz
curl -s -c /tmp/ad-cookies.txt -X POST http://127.0.0.1:8080/api/v1/auth/login \
  -H 'Content-Type: application/json' -d '{"username":"admin","password":"secret123"}'
curl -s -b /tmp/ad-cookies.txt http://127.0.0.1:8080/api/v1/auth/me
curl -s http://127.0.0.1:8080/api/v1/auth/me
```

Expected:
- healthz/readyz 返回 `{"status":"ok"}` / `{"status":"ready"}`
- login 返回 200 并携带 `Set-Cookie: agentdocs_session=...; HttpOnly`
- 带 cookie 的 me 返回 `{"user":{"id":"usr_...","username":"admin","is_admin":true,...}}`
- 不带 cookie 的 me 返回 401 错误信封

重启验证（验收点 3）：

```bash
kill %1
sleep 1
./agentdocs serve > /tmp/agentdocs.log 2>&1 &
sleep 1
curl -s -b /tmp/ad-cookies.txt http://127.0.0.1:8080/api/v1/auth/me
kill %1
```

Expected: 重启后同一 cookie 仍然返回 200 —— Session 在 SQLite 中持久化。

- [ ] **Step 5: 提交**

```bash
gofmt -l .
git add cmd/
git commit -m "feat: add agentdocs cli (serve, admin create)"
```

### Task 14: 前端脚手架（Vite + Tailwind + shadcn/ui + Vitest）

**Files:**
- Create: `web/`（脚手架生成 + 依赖 + 配置）
- Modify: `web/.gitignore`（保留占位符）、`web/package.json`（scripts）

- [ ] **Step 1: 生成脚手架（临时目录避免与 embed.go/dist 冲突）**

```bash
cd /home/choken/code/xwiki
npm create -y vite@latest web-tmp -- --template react-ts
cp -a web-tmp/. web/
rm -rf web-tmp
```

Expected: `web/` 下生成 Vite + React + TS 模板；`web/embed.go` 与 `web/dist/index.html` 保留不动。

- [ ] **Step 2: 安装依赖**

```bash
cd web
npm install
npm install react-router-dom @tanstack/react-query zustand react-hook-form zod @hookform/resolvers lucide-react
npm install -D vitest @testing-library/react @testing-library/jest-dom @testing-library/user-event jsdom @tailwindcss/vite
```

Expected: 全部成功，生成 `package-lock.json`。

- [ ] **Step 3: 初始化 Tailwind v4 + shadcn/ui**

```bash
npx shadcn@latest init -y -b neutral
npx shadcn@latest add button card input label alert sonner
```

Expected: `src/index.css` 含 `@import "tailwindcss"` 与主题变量；`src/components/ui/` 下生成 button、card、input、label、alert、sonner 组件。
若 CLI 出现交互提示（版本差异），一律接受默认值；若 `init` 失败，手动执行：
`npm install -D tailwindcss @tailwindcss/vite`，在 `vite.config.ts` 加 `tailwindcss()` 插件，`src/index.css` 写入 `@import "tailwindcss";`，并手工复制 shadcn 组件。

- [ ] **Step 4: 调整 web/.gitignore 保留占位符**

`web/.gitignore` 中把 `dist` 替换为：

```gitignore
dist/*
!dist/index.html
```

Expected: `web/dist/index.html` 保持被跟踪（go:embed 依赖它）。

- [ ] **Step 5: 更新 package.json scripts**

`web/package.json` 的 `scripts` 增加：

```json
"test": "vitest run",
"test:watch": "vitest"
```

- [ ] **Step 6: 基线验证**

```bash
npm run build
```

Expected: `tsc -b && vite build` 成功，产物在 `web/dist/`。

- [ ] **Step 7: 提交**

```bash
cd /home/choken/code/xwiki
git add web/
git commit -m "feat: scaffold web frontend (vite, tailwind, shadcn/ui)"
```

注：`web/dist/index.html` 会被构建覆盖，提交前不需要还原（Task 17 会统一处理占位符策略）。

### Task 15: 前端 API 客户端与认证状态（TDD）

**Files:**
- Create: `web/src/lib/api/types.ts`、`web/src/lib/api/client.ts`、`web/src/lib/api/auth.ts`、`web/src/stores/auth.ts`、`web/src/test/setup.ts`
- Test: `web/src/lib/api/client.test.ts`、`web/src/stores/auth.test.ts`
- Modify: `web/vite.config.ts`（alias + proxy + vitest 配置）、`web/tsconfig.app.json`（paths）

- [ ] **Step 1: 配置 vite.config.ts 与 tsconfig**

`web/vite.config.ts` 整体替换为：

```ts
/// <reference types="vitest/config" />
import { fileURLToPath, URL } from "node:url";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  server: {
    proxy: {
      "/api": "http://localhost:8080",
    },
  },
  test: {
    environment: "jsdom",
    setupFiles: "./src/test/setup.ts",
  },
});
```

`web/tsconfig.app.json` 的 `compilerOptions` 中增加：

```json
"baseUrl": ".",
"paths": { "@/*": ["./src/*"] }
```

`web/src/test/setup.ts`：

```ts
import "@testing-library/jest-dom/vitest";
```

- [ ] **Step 2: 写失败测试（client）**

`web/src/lib/api/client.test.ts`：

```ts
import { afterEach, describe, expect, it, vi } from "vitest";
import { api, ApiError } from "./client";

describe("api client", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("returns parsed JSON on success", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        json: async () => ({ user: { username: "admin" } }),
      })
    );
    await expect(
      api<{ user: { username: string } }>("/auth/me")
    ).resolves.toEqual({ user: { username: "admin" } });
  });

  it("throws ApiError with the server error code", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 409,
        json: async () => ({
          error: { code: "revision_conflict", message: "conflict" },
        }),
      })
    );
    const err = await api("/x").catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    if (err instanceof ApiError) {
      expect(err.status).toBe(409);
      expect(err.code).toBe("revision_conflict");
    }
  });

  it("falls back to internal_error on non-JSON errors", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 500,
        json: async () => {
          throw new Error("not json");
        },
      })
    );
    const err = await api("/x").catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    if (err instanceof ApiError) {
      expect(err.code).toBe("internal_error");
    }
  });
});
```

- [ ] **Step 3: 运行测试确认失败**

```bash
cd web && npx vitest run src/lib/api/client.test.ts
```

Expected: FAIL（`./client` 不存在）。

- [ ] **Step 4: 实现 client 与类型**

`web/src/lib/api/types.ts`：

```ts
export interface User {
  id: string;
  username: string;
  display_name: string;
  is_admin: boolean;
}

export interface AuthResponse {
  user: User;
}
```

`web/src/lib/api/client.ts`：

```ts
export class ApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
  }
}

interface ErrorEnvelope {
  error?: { code?: string; message?: string };
}

const BASE = "/api/v1";

export async function api<T>(path: string, init: RequestInit = {}): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    ...init,
    credentials: "include",
    headers: {
      "Content-Type": "application/json",
      ...init.headers,
    },
  });
  if (!res.ok) {
    let code = "internal_error";
    let message = `请求失败（HTTP ${res.status}）`;
    try {
      const body = (await res.json()) as ErrorEnvelope;
      code = body.error?.code ?? code;
      message = body.error?.message ?? message;
    } catch {
      // non-JSON error body
    }
    throw new ApiError(res.status, code, message);
  }
  return (await res.json()) as T;
}
```

`web/src/lib/api/auth.ts`：

```ts
import { api } from "./client";
import type { AuthResponse } from "./types";

export function login(username: string, password: string) {
  return api<AuthResponse>("/auth/login", {
    method: "POST",
    body: JSON.stringify({ username, password }),
  });
}

export function logout() {
  return api<{ ok: boolean }>("/auth/logout", { method: "POST" });
}

export function me() {
  return api<AuthResponse>("/auth/me");
}

export function changePassword(current_password: string, new_password: string) {
  return api<{ ok: boolean }>("/auth/password", {
    method: "POST",
    body: JSON.stringify({ current_password, new_password }),
  });
}
```

- [ ] **Step 5: 运行 client 测试确认通过**

```bash
npx vitest run src/lib/api/client.test.ts
```

Expected: 3 tests PASS。

- [ ] **Step 6: 写失败测试（store）**

`web/src/stores/auth.test.ts`：

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as authApi from "@/lib/api/auth";
import { useAuthStore } from "./auth";

vi.mock("@/lib/api/auth", () => ({
  login: vi.fn(),
  logout: vi.fn(),
  me: vi.fn(),
}));

const adminUser = {
  id: "usr_1",
  username: "admin",
  display_name: "admin",
  is_admin: true,
};

describe("useAuthStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAuthStore.setState({ user: null, initializing: true });
  });

  it("login sets the current user", async () => {
    vi.mocked(authApi.login).mockResolvedValue({ user: adminUser });
    await useAuthStore.getState().login("admin", "secret123");
    expect(useAuthStore.getState().user?.username).toBe("admin");
  });

  it("login failure leaves the user null", async () => {
    vi.mocked(authApi.login).mockRejectedValue(new Error("invalid credentials"));
    await expect(
      useAuthStore.getState().login("admin", "bad")
    ).rejects.toThrow();
    expect(useAuthStore.getState().user).toBeNull();
  });

  it("fetchMe restores the session", async () => {
    vi.mocked(authApi.me).mockResolvedValue({ user: adminUser });
    await useAuthStore.getState().fetchMe();
    expect(useAuthStore.getState().user?.username).toBe("admin");
    expect(useAuthStore.getState().initializing).toBe(false);
  });

  it("fetchMe without session clears user", async () => {
    vi.mocked(authApi.me).mockRejectedValue(new Error("unauthorized"));
    await useAuthStore.getState().fetchMe();
    expect(useAuthStore.getState().user).toBeNull();
    expect(useAuthStore.getState().initializing).toBe(false);
  });
});
```

- [ ] **Step 7: 运行测试确认失败**

```bash
npx vitest run src/stores/auth.test.ts
```

Expected: FAIL（`./auth` 不存在）。

- [ ] **Step 8: 实现 store**

`web/src/stores/auth.ts`：

```ts
import { create } from "zustand";
import {
  login as apiLogin,
  logout as apiLogout,
  me as apiMe,
} from "@/lib/api/auth";
import type { User } from "@/lib/api/types";

interface AuthState {
  user: User | null;
  initializing: boolean;
  login: (username: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  fetchMe: () => Promise<void>;
}

export const useAuthStore = create<AuthState>((set) => ({
  user: null,
  initializing: true,
  login: async (username, password) => {
    const res = await apiLogin(username, password);
    set({ user: res.user });
  },
  logout: async () => {
    await apiLogout();
    set({ user: null });
  },
  fetchMe: async () => {
    try {
      const res = await apiMe();
      set({ user: res.user, initializing: false });
    } catch {
      set({ user: null, initializing: false });
    }
  },
}));
```

- [ ] **Step 9: 运行全部前端测试 + 提交**

```bash
npx vitest run
```

Expected: 全部 PASS（client 3 个 + store 4 个）。

```bash
cd /home/choken/code/xwiki
git add web/src web/vite.config.ts web/tsconfig.app.json
git commit -m "feat: add frontend api client and auth store"
```

### Task 16: 登录页、路由守卫与布局（TDD）

**Files:**
- Create: `web/src/routes/login.tsx`、`web/src/routes/home.tsx`、`web/src/components/layout/protected.tsx`、`web/src/app/router.tsx`
- Create: `web/src/routes/login.test.tsx`
- Modify: `web/src/main.tsx`；删除模板 `web/src/App.tsx`、`web/src/App.css`、`web/src/assets/react.svg`

- [ ] **Step 1: 写失败测试（登录页）**

`web/src/routes/login.test.tsx`：

```tsx
import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import LoginPage from "./login";
import * as authApi from "@/lib/api/auth";

vi.mock("@/lib/api/auth", () => ({
  login: vi.fn(),
}));

function renderPage() {
  return render(
    <MemoryRouter initialEntries={["/login"]}>
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route path="/" element={<div>home-page</div>} />
      </Routes>
    </MemoryRouter>
  );
}

describe("LoginPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders username and password fields", () => {
    renderPage();
    expect(screen.getByLabelText("用户名")).toBeInTheDocument();
    expect(screen.getByLabelText("密码")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "登录" })).toBeInTheDocument();
  });

  it("shows an error alert when login fails", async () => {
    vi.mocked(authApi.login).mockRejectedValue(
      new Error("invalid credentials")
    );
    const user = userEvent.setup();
    renderPage();
    await user.type(screen.getByLabelText("用户名"), "admin");
    await user.type(screen.getByLabelText("密码"), "wrongpass");
    await user.click(screen.getByRole("button", { name: "登录" }));
    expect(await screen.findByRole("alert")).toBeInTheDocument();
  });

  it("navigates to home after successful login", async () => {
    vi.mocked(authApi.login).mockResolvedValue({
      user: {
        id: "usr_1",
        username: "admin",
        display_name: "admin",
        is_admin: true,
      },
    });
    const user = userEvent.setup();
    renderPage();
    await user.type(screen.getByLabelText("用户名"), "admin");
    await user.type(screen.getByLabelText("密码"), "secret123");
    await user.click(screen.getByRole("button", { name: "登录" }));
    expect(await screen.findByText("home-page")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cd web && npx vitest run src/routes/login.test.tsx
```

Expected: FAIL（`./login` 不存在）。

- [ ] **Step 3: 实现登录页**

`web/src/routes/login.tsx`：

```tsx
import { useState } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useAuthStore } from "@/stores/auth";

const schema = z.object({
  username: z.string().min(1, "请输入用户名"),
  password: z.string().min(1, "请输入密码"),
});

type FormValues = z.infer<typeof schema>;

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
    <div className="flex min-h-screen items-center justify-center bg-muted/40 p-4">
      <Card className="w-full max-w-sm">
        <CardHeader>
          <CardTitle>AgentDocs</CardTitle>
          <CardDescription>登录以继续</CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={onSubmit} className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="username">用户名</Label>
              <Input id="username" autoComplete="username" {...register("username")} />
              {errors.username && (
                <p className="text-sm text-destructive">{errors.username.message}</p>
              )}
            </div>
            <div className="space-y-2">
              <Label htmlFor="password">密码</Label>
              <Input
                id="password"
                type="password"
                autoComplete="current-password"
                {...register("password")}
              />
              {errors.password && (
                <p className="text-sm text-destructive">{errors.password.message}</p>
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
        </CardContent>
      </Card>
    </div>
  );
}
```

- [ ] **Step 4: 运行登录页测试确认通过**

```bash
npx vitest run src/routes/login.test.tsx
```

Expected: 3 tests PASS。

- [ ] **Step 5: 实现路由守卫、首页与路由表**

`web/src/components/layout/protected.tsx`：

```tsx
import { useEffect } from "react";
import { Navigate, Outlet } from "react-router-dom";
import { useAuthStore } from "@/stores/auth";

export default function ProtectedRoute() {
  const user = useAuthStore((s) => s.user);
  const initializing = useAuthStore((s) => s.initializing);
  const fetchMe = useAuthStore((s) => s.fetchMe);

  useEffect(() => {
    void fetchMe();
  }, [fetchMe]);

  if (initializing) {
    return (
      <div className="flex h-screen items-center justify-center text-muted-foreground">
        加载中…
      </div>
    );
  }
  if (!user) {
    return <Navigate to="/login" replace />;
  }
  return <Outlet />;
}
```

`web/src/routes/home.tsx`：

```tsx
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
```

`web/src/app/router.tsx`：

```tsx
import { createBrowserRouter } from "react-router-dom";
import ProtectedRoute from "@/components/layout/protected";
import HomePage from "@/routes/home";
import LoginPage from "@/routes/login";

export const router = createBrowserRouter([
  { path: "/login", element: <LoginPage /> },
  {
    path: "/",
    element: <ProtectedRoute />,
    children: [{ index: true, element: <HomePage /> }],
  },
]);
```

- [ ] **Step 6: 重写 main.tsx 并清理模板文件**

`web/src/main.tsx` 整体替换为：

```tsx
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "react-router-dom";
import { Toaster } from "@/components/ui/sonner";
import { router } from "@/app/router";
import "@/index.css";

const queryClient = new QueryClient();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
      <Toaster richColors />
    </QueryClientProvider>
  </StrictMode>
);
```

删除模板文件：

```bash
rm web/src/App.tsx web/src/App.css web/src/assets/react.svg
rmdir web/src/assets
```

- [ ] **Step 7: 类型检查 + 全部前端测试**

```bash
npm run build
npx vitest run
```

Expected: 构建成功；全部测试 PASS（client 3 + store 4 + login 3）。

- [ ] **Step 8: 提交**

```bash
cd /home/choken/code/xwiki
git add web/src
git commit -m "feat: add login page, routing and protected layout"
```

### Task 17: 端到端冒烟测试（前端 + 后端一体）

**Files:**
- 无新增文件；验证构建链路与部署形态

- [ ] **Step 1: 构建前端并嵌入二进制**

```bash
cd web
npm run build
cd ..
go build -o agentdocs ./cmd/agentdocs
```

Expected: `web/dist` 生成真实 bundle；二进制构建成功（已嵌入真实前端）。

- [ ] **Step 2: 一体冒烟**

```bash
./agentdocs serve > /tmp/agentdocs.log 2>&1 &
sleep 1
curl -s http://127.0.0.1:8080/ | head -5
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/login
curl -s -c /tmp/ad-cookies.txt -X POST http://127.0.0.1:8080/api/v1/auth/login \
  -H 'Content-Type: application/json' -d '{"username":"admin","password":"secret123"}'
curl -s -b /tmp/ad-cookies.txt http://127.0.0.1:8080/api/v1/auth/me
kill %1
```

Expected:
- `/` 返回真实 index.html（含 `<div id="root">`），不再是占位符
- `/login` 返回 200（SPA fallback）
- login/me 正常（cookie 会话）

- [ ] **Step 3: 恢复占位符策略**

```bash
git restore web/dist/index.html
```

Expected: `web/dist/index.html` 恢复为占位符版本（仓库中始终只跟踪占位符；Docker 构建与本地开发都会重新生成真实产物）。

- [ ] **Step 4: 浏览器人工验证（可选）**

若环境允许：启动 `./agentdocs serve`，浏览器打开 `http://localhost:8080`，用 admin 登录，确认跳转到首页并显示「已登录：admin」；刷新页面确认会话保持（验收点 2）。

### Task 18: 项目文档

**Files:**
- Create: `README.md`、`doc/architecture.md`、`doc/api.md`、`doc/development.md`

- [ ] **Step 1: 编写 README.md**

`README.md`（仓库根）：

```markdown
# AgentDocs

面向人类与 AI Agent 的轻量 Git 文档管理系统。

> 当前为阶段一（项目骨架）。完整需求见 `doc/spec.md`，分阶段实施计划见 `doc/plans/`。

## 快速开始

前置：Go 1.26+、Node 26+（构建前端）、Git。

### 开发模式

```bash
# 后端
./agentdocs serve   # 或 go run ./cmd/agentdocs serve
# 前端（开发服务器，/api 代理到 :8080）
cd web && npm run dev
```

### 首次使用

```bash
./agentdocs admin create -username admin -password secret123
```

浏览器打开 http://localhost:8080 登录。

### 构建与测试

```bash
cd web && npm install && npm run build && cd ..
go build -o agentdocs ./cmd/agentdocs
go test ./...
cd web && npm run test
```

### Docker

```bash
docker compose up -d --build
```

## 配置（环境变量）

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `AGENTDOCS_DATA_DIR` | `data` | 数据目录（SQLite + 仓库） |
| `AGENTDOCS_HTTP_ADDR` | `:8080` | HTTP 监听地址 |
| `AGENTDOCS_WEB_ORIGIN` | `http://localhost:5173` | 允许的 CORS 来源 |
| `AGENTDOCS_SESSION_TTL` | `720h` | 会话有效期 |
| `AGENTDOCS_MAX_BODY_BYTES` | `1048576` | 请求体上限 |
| `AGENTDOCS_COOKIE_SECURE` | `false` | 生产环境设为 true |

## 目录结构

见 `doc/architecture.md`。
```

- [ ] **Step 2: 编写 doc/architecture.md**

`doc/architecture.md` 要点（用中文完整展开，至少包含）：

```markdown
# 架构说明（阶段一）

## 分层

HTTP 层（internal/httpapi）→ 服务层（internal/auth、internal/app）→ 存储层（internal/user、internal/store/sqlite）。

- 路由：internal/server/router.go（chi）
- 中间件链：RequestID → RequestLogger → Recoverer → CORS → 路由
- 认证：SessionAuth 中间件解析 HttpOnly Cookie（只存 SHA-256 哈希）
- 密码：Argon2id（PHC 格式），见 internal/auth/password.go
- 错误：统一信封 {error:{code,message,request_id}}，见 internal/httpapi/response

## 请求生命周期

1. RequestID 中间件生成/透传 request_id（响应头 X-Request-ID）
2. RequestLogger 输出一行 JSON 日志
3. 处理器解码请求体（限制大小、严格字段）
4. 服务层完成业务，写 SQLite（WAL）
5. response 包统一序列化

## 数据存储

- SQLite：data/agentdocs.db（WAL、外键、busy_timeout）
- 迁移：goose，SQL 文件内嵌于二进制（internal/store/sqlite/migrations）
- 表：users、sessions、schema_migrations

## 前端

- Vite + React + shadcn/ui，构建产物嵌入 Go 二进制（web/embed.go）
- 会话恢复：ProtectedRoute 挂载时调用 GET /api/v1/auth/me
- 开发模式：Vite 代理 /api → :8080，无 CORS 问题

## 安全基线（对应 spec §21）

- 密码哈希 Argon2id；会话 Token 只存哈希
- Cookie：HttpOnly + SameSite=Lax（生产加 Secure）
- 请求体大小限制；未知字段拒绝
- 不记录 Token、密码、会话内容（spec §25）
```

- [ ] **Step 3: 编写 doc/api.md**

`doc/api.md`（用中文完整展开，至少包含）：

```markdown
# REST API（阶段一）

前缀：/api/v1。错误统一为 {error:{code,message,request_id}}。

## 健康检查

- GET /healthz → 200 {"status":"ok"}
- GET /readyz → 200 {"status":"ready"} / 503

## 认证

### POST /auth/login

请求：{"username":"...","password":"..."}

- 200 {"user":{...}}，Set-Cookie: agentdocs_session（HttpOnly）
- 401 invalid_credentials

### POST /auth/logout

清除会话（需 cookie）。→ 200 {"ok":true}

### GET /auth/me（需登录）

→ 200 {"user":{id,username,display_name,is_admin}}

### POST /auth/password（需登录）

请求：{"current_password":"...","new_password":"..."}

- 200 {"ok":true}；401 invalid_credentials；400 validation_failed

## 错误码（本阶段使用）

validation_failed / invalid_credentials / authentication_required / not_found / not_ready / internal_error

## curl 示例

```bash
curl -c /tmp/cj.txt -X POST http://localhost:8080/api/v1/auth/login \
  -H 'Content-Type: application/json' -d '{"username":"admin","password":"secret123"}'
curl -b /tmp/cj.txt http://localhost:8080/api/v1/auth/me
```
```

- [ ] **Step 4: 编写 doc/development.md**

`doc/development.md` 要点（用中文完整展开）：

```markdown
# 开发指南

## 前置

Go 1.26+、Node 26+、Git（运行时也需要 Git，阶段二起使用）。

## 常用命令

- 后端测试：go test ./...
- 前端测试：cd web && npm run test
- 前端构建：cd web && npm run build（必须先于 go build，产物被 go:embed）
- 编译：go build -o agentdocs ./cmd/agentdocs

## 新增迁移

在 internal/store/sqlite/migrations 增加 000NN_*.sql，goose 在启动时自动执行。

## 占位符策略

web/dist/index.html 是提交到仓库的占位符，保证 fresh checkout 时 go:embed 可编译；真实产物由 npm run build 或 Docker 生成，不提交。

## 前端开发

npm run dev（:5173），/api 代理到 :8080。shadcn/ui 组件：npx shadcn@latest add <name>。

## 约定

- 所有 Git 命令必须经由 internal/gitrepo 封装（阶段二引入），业务层禁止直接 exec.Command
- 提交信息遵循 conventional commits（feat/fix/chore/docs）
```

- [ ] **Step 5: 提交**

```bash
git add README.md doc/architecture.md doc/api.md doc/development.md
git commit -m "docs: add readme and architecture/api/development docs"
```

### Task 19: Docker 部署

**Files:**
- Create: `Dockerfile`、`.dockerignore`、`docker-compose.yml`

- [ ] **Step 1: 编写 Dockerfile**

`Dockerfile`：

```dockerfile
# ---- Frontend build ----
FROM node:26-alpine AS web
WORKDIR /app
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# ---- Go build ----
FROM golang:1.26-alpine AS build
WORKDIR /src
COPY go.mod go.sum ./
RUN go mod download
COPY . .
COPY --from=web /app/dist ./web/dist
RUN CGO_ENABLED=0 go build -o /out/agentdocs ./cmd/agentdocs

# ---- Runtime ----
FROM alpine:3.22
RUN apk add --no-cache git ca-certificates
COPY --from=build /out/agentdocs /usr/local/bin/agentdocs
ENV AGENTDOCS_DATA_DIR=/data
EXPOSE 8080
ENTRYPOINT ["agentdocs"]
CMD ["serve"]
```

注：运行时镜像安装 `git`（阶段二起需要）；若本地 Go/Node 版本与镜像 tag 不一致，先 `docker pull golang:1.26-alpine` 确认存在，否则降级到对应稳定 tag。

- [ ] **Step 2: 编写 .dockerignore**

`.dockerignore`：

```gitignore
.git
.atl
data
agentdocs
web/node_modules
web/dist
```

- [ ] **Step 3: 编写 docker-compose.yml**

`docker-compose.yml`：

```yaml
services:
  agentdocs:
    build: .
    ports:
      - "8080:8080"
    environment:
      AGENTDOCS_HTTP_ADDR: ":8080"
    volumes:
      - agentdocs-data:/data

volumes:
  agentdocs-data:
```

- [ ] **Step 4: 构建并验证**

```bash
docker compose up -d --build
sleep 3
curl -s http://127.0.0.1:8080/healthz
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/
docker compose exec agentdocs agentdocs admin create -username admin -password secret123
curl -s -c /tmp/ad-cookies.txt -X POST http://127.0.0.1:8080/api/v1/auth/login \
  -H 'Content-Type: application/json' -d '{"username":"admin","password":"secret123"}'
curl -s -b /tmp/ad-cookies.txt http://127.0.0.1:8080/api/v1/auth/me
docker compose down
```

Expected: healthz ok；`/` 返回真实前端（含 `<div id="root">`）；容器内 CLI 可创建管理员；login/me 正常（验收点 20 的 Compose 启动能力）。

- [ ] **Step 5: 提交**

```bash
git add Dockerfile .dockerignore docker-compose.yml
git commit -m "feat: add docker deployment"
```

### Task 20: 阶段一验收清单

**Files:**
- Modify: `doc/plans/README.md`（状态改为已完成）

- [ ] **Step 1: 全量回归**

```bash
go vet ./...
go test ./... -count=1
cd web && npx vitest run && cd ..
```

Expected: vet 无告警；Go 测试全绿（含会话重启持久化）；前端测试全绿。

- [ ] **Step 2: 最终构建**

```bash
cd web && npm run build && cd ..
git restore web/dist/index.html
go build -o agentdocs ./cmd/agentdocs
```

- [ ] **Step 3: 对照 spec 勾选验收项**

阶段一验收（spec §27）：

- [x] 可以创建管理员（CLI）
- [x] 可以登录（网页 + API）
- [x] 服务重启后 Session 和数据库正常

- [ ] **Step 4: 更新计划索引并提交**

在 `doc/plans/README.md` 将阶段 01 状态改为「✅ 已完成」，然后：

```bash
git add doc/plans/README.md
git commit -m "docs: mark phase 1 plan complete"
```

## 3. 给执行者的注意事项

1. **严格按步骤顺序执行**；TDD 步骤（先写测试 → 确认失败 → 实现 → 确认通过）不可颠倒。
2. 每个任务结束必须提交；提交信息使用计划中给定的原文。
3. `go test ./...` 需要 `web/dist/index.html` 占位符存在——它从 Task 11 起就在仓库中。
4. 若 `goose.SetTableName` 在当前 goose 版本编译失败，删除该行并改用默认表 `goose_db_version`，同步更新 Task 5 测试与文档中的表名。
5. 若 shadcn CLI 交互行为与计划不一致，接受默认值即可；Tailwind v4 插件（@tailwindcss/vite）必须保留在 vite.config.ts 中。
6. 阶段一不引入 JWT、不建 pages/folders 表、不做 Git 操作（spec §28 与阶段二分界）。
7. 执行完成后，建议按 `doc/plans/README.md` 的顺序启动阶段二计划编写。
