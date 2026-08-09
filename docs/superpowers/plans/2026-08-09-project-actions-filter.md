# 项目操作菜单与状态筛选 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为首页项目卡片增加三点操作菜单（重命名、删除、归档/恢复）和项目状态筛选。

**Architecture:** 复用后端已有的项目 PATCH、DELETE、archive 和 unarchive 接口。首页在本地维护状态筛选和重命名 Dialog 状态；Base UI Menu 负责菜单的键盘导航、焦点和外部关闭，现有 Dialog 负责重命名表单。

**Tech Stack:** React 19、React Router、TanStack Query、Base UI Menu、Radix Dialog、Vitest、Testing Library、Tailwind CSS。

---

### Task 1: 补充项目 API 客户端

**Files:**
- Modify: `web/src/lib/api/projects.test.ts`
- Modify: `web/src/lib/api/projects.ts`

- [ ] **Step 1: Write the failing API tests**

在 `projects.test.ts` 的 import 中加入 `deleteProject` 和 `renameProject`，并添加：

```ts
it("renames a project via PATCH", async () => {
  mockFetchOnce(200, { project: { id: "prj_1", name: "new-name" } });
  const res = await renameProject("prj_1", "new-name");
  expect(res.project.name).toBe("new-name");
  const [url, init] = (fetch as ReturnType<typeof vi.fn>).mock.calls[0] as [string, RequestInit];
  expect(url).toBe("/api/v1/projects/prj_1");
  expect(init.method).toBe("PATCH");
  expect(JSON.parse(String(init.body))).toEqual({ name: "new-name" });
});

it("deletes a project via DELETE", async () => {
  mockFetchOnce(200, { deleted: true });
  await expect(deleteProject("prj_1")).resolves.toEqual({ deleted: true });
  const [url, init] = (fetch as ReturnType<typeof vi.fn>).mock.calls[0] as [string, RequestInit];
  expect(url).toBe("/api/v1/projects/prj_1");
  expect(init.method).toBe("DELETE");
});
```

- [ ] **Step 2: Run the focused tests and verify they fail for missing exports**

Run:

```bash
pnpm --dir web exec vitest run src/lib/api/projects.test.ts
```

Expected: FAIL because `renameProject` and `deleteProject` are not exported yet.

- [ ] **Step 3: Implement the two API functions**

Append to `web/src/lib/api/projects.ts`:

```ts
export function renameProject(id: string, name: string) {
  return api<ProjectResponse>(`/projects/${encodeURIComponent(id)}`, {
    method: "PATCH",
    body: JSON.stringify({ name }),
  });
}

export function deleteProject(id: string) {
  return api<{ deleted: boolean }>(`/projects/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
}
```

- [ ] **Step 4: Run the focused tests and verify they pass**

Run:

```bash
pnpm --dir web exec vitest run src/lib/api/projects.test.ts
```

Expected: all project API tests pass.

---

### Task 2: Add the project three-dot action menu and rename Dialog

**Files:**
- Modify: `web/src/routes/home.test.tsx`
- Modify: `web/src/routes/home.tsx`

- [ ] **Step 1: Extend the home API mocks and write failing interaction tests**

Add these functions to the existing `vi.mock("@/lib/api/projects", ...)` object:

```ts
renameProject: vi.fn(),
deleteProject: vi.fn(),
unarchiveProject: vi.fn(),
```

Update the archive test to open the project action trigger first, then click the `归档` menu item. Add tests covering:

```tsx
it("opens project actions and renames a project", async () => {
  vi.mocked(projectsApi.listProjects).mockResolvedValue({ projects: [sampleProject()] });
  vi.mocked(projectsApi.renameProject).mockResolvedValue({
    project: sampleProject({ name: "renamed" }),
  });
  const user = userEvent.setup();
  renderPage();

  await user.click(await screen.findByRole("button", { name: "项目操作 docs-site" }));
  expect(screen.getByRole("menuitem", { name: "重命名" })).toBeInTheDocument();
  expect(screen.getByRole("menuitem", { name: "删除" })).toBeInTheDocument();
  expect(screen.getByRole("menuitem", { name: "归档" })).toBeInTheDocument();

  await user.click(screen.getByRole("menuitem", { name: "重命名" }));
  const input = screen.getByLabelText("项目名");
  await user.clear(input);
  await user.type(input, "renamed");
  await user.click(screen.getByRole("button", { name: "保存" }));

  await vi.waitFor(() =>
    expect(projectsApi.renameProject).toHaveBeenCalledWith("prj_1", "renamed"),
  );
});

it("confirms before deleting a project", async () => {
  vi.spyOn(window, "confirm").mockReturnValue(true);
  vi.mocked(projectsApi.listProjects).mockResolvedValue({ projects: [sampleProject()] });
  vi.mocked(projectsApi.deleteProject).mockResolvedValue({ deleted: true });
  const user = userEvent.setup();
  renderPage();

  await user.click(await screen.findByRole("button", { name: "项目操作 docs-site" }));
  await user.click(screen.getByRole("menuitem", { name: "删除" }));

  expect(window.confirm).toHaveBeenCalledWith(expect.stringContaining("docs-site"));
  await vi.waitFor(() => expect(projectsApi.deleteProject).toHaveBeenCalledWith("prj_1"));
});
```

Also assert the archived project menu exposes `恢复` instead of `归档`.

- [ ] **Step 2: Run the focused home tests and verify the new tests fail**

Run:

```bash
pnpm --dir web exec vitest run src/routes/home.test.tsx
```

Expected: FAIL because the current card still renders the archive button and has no menu/Dialog.

- [ ] **Step 3: Implement the menu and rename flow**

In `home.tsx`:

1. Import `Menu` from `@base-ui/react/menu`, icons `MoreHorizontal`, `Pencil`, and `Trash2`, existing Dialog/Input/Label components, `ApiError`, `renameProject`, and `deleteProject`.
2. Replace the active/archive Button in `ProjectCard` with a `Menu.Root` whose trigger has `aria-label={`项目操作 ${project.name}`}` and shows `MoreHorizontal`. Prevent the trigger click from navigating the surrounding project link.
3. Render the menu through `Menu.Portal`, `Menu.Positioner`, and `Menu.Popup`. Use `Menu.Item` for `重命名`, `删除`, and `归档`/`恢复`; use a destructive color for `删除`.
4. Keep the existing archive/unarchive calls and query invalidation. Add handlers with the same toast/error pattern:

```ts
const onDelete = async () => {
  if (!window.confirm(`确认删除项目「${project.name}」？这将删除项目及其 Git 仓库，且无法恢复。`)) return;
  setBusy(true);
  try {
    await deleteProject(project.id);
    toast.success(`项目 ${project.name} 已删除`);
    await queryClient.invalidateQueries({ queryKey: ["projects"] });
  } catch (err) {
    toast.error(err instanceof Error ? err.message : "删除失败");
  } finally {
    setBusy(false);
  }
};
```

5. Add a controlled Dialog for renaming. Initialize the input to `project.name`; accept only 1–64 characters matching `/^[a-z0-9]+(-[a-z0-9]+)*$/`; show the validation message inline; call `renameProject(project.id, value)` on save; map `ApiError` code `project_name_conflict` to `同名项目已存在`; close and invalidate the projects query after success.
6. Keep the existing card navigation and metadata overflow fix unchanged.

- [ ] **Step 4: Run the focused home tests and verify they pass**

Run:

```bash
pnpm --dir web exec vitest run src/routes/home.test.tsx
```

Expected: all home tests pass, including menu, rename, delete confirmation, archive, and restore behavior.

---

### Task 3: Add the project status filter

**Files:**
- Modify: `web/src/routes/home.test.tsx`
- Modify: `web/src/routes/home.tsx`

- [ ] **Step 1: Write the failing filter tests**

Add a test with one active and one archived project:

```tsx
it("filters projects by status", async () => {
  vi.mocked(projectsApi.listProjects).mockResolvedValue({
    projects: [
      sampleProject({ id: "prj_active", name: "active-project" }),
      sampleProject({ id: "prj_archived", name: "archived-project", archived: true }),
    ],
  });
  const user = userEvent.setup();
  renderPage();

  const filter = await screen.findByRole("combobox", { name: "项目状态" });
  await user.selectOptions(filter, "archived");
  expect(screen.getByText("archived · 1")).toBeInTheDocument();
  expect(screen.queryByText("active · 1")).not.toBeInTheDocument();

  await user.selectOptions(filter, "active");
  expect(screen.getByText("active · 1")).toBeInTheDocument();
  expect(screen.queryByText("archived · 1")).not.toBeInTheDocument();
});
```

Add a test that selecting a status with no matching projects renders `暂无符合条件的项目` without rendering the initial “还没有项目” state.

- [ ] **Step 2: Run the focused home tests and verify the filter tests fail**

Run:

```bash
pnpm --dir web exec vitest run src/routes/home.test.tsx
```

Expected: FAIL because there is no status combobox and the page always renders both sections.

- [ ] **Step 3: Implement local filtering**

Add the state and derived arrays in `HomePage`:

```ts
type ProjectStatusFilter = "all" | "active" | "archived";
const [statusFilter, setStatusFilter] = useState<ProjectStatusFilter>("all");
const visibleActive = statusFilter === "archived" ? [] : active;
const visibleArchived = statusFilter === "active" ? [] : archived;
const hasProjects = active.length > 0 || archived.length > 0;
const hasVisibleProjects = visibleActive.length > 0 || visibleArchived.length > 0;
```

Add a labeled native select near the page actions:

```tsx
<label className="flex items-center gap-2 text-sm text-[var(--color-ink-3)]">
  <span>项目状态</span>
  <select
    aria-label="项目状态"
    value={statusFilter}
    onChange={(event) => setStatusFilter(event.target.value as ProjectStatusFilter)}
    className="h-8 rounded-[var(--radius)] border border-[var(--color-rule)] bg-[var(--color-paper)] px-2 text-sm text-[var(--color-ink-2)]"
  >
    <option value="all">全部</option>
    <option value="active">活跃</option>
    <option value="archived">已归档</option>
  </select>
</label>
```

Render sections from `visibleActive`/`visibleArchived`. Keep the current true-empty state only when `!hasProjects`; render a separate `hairline-panel` with `暂无符合条件的项目` when `hasProjects && !hasVisibleProjects`.

- [ ] **Step 4: Run the focused home tests and verify they pass**

Run:

```bash
pnpm --dir web exec vitest run src/routes/home.test.tsx
```

Expected: all home tests pass.

---

### Task 4: Full verification

**Files:**
- No additional source files.

- [ ] **Step 1: Run the complete web test suite**

Run:

```bash
pnpm --dir web test
```

Expected: 0 failed test files and 0 failed tests.

- [ ] **Step 2: Run lint and production build**

Run:

```bash
pnpm --dir web lint
pnpm --dir web build
```

Expected: lint exits successfully (existing warnings may remain), TypeScript compilation succeeds, and Vite production build succeeds.

- [ ] **Step 3: Inspect the final diff**

Run:

```bash
git diff --check
git status --short
```

Confirm the feature changes are limited to the API client, home route, and their tests; do not stage unrelated existing working-tree changes such as `web/dist/index.html` or prior API-docs/Vite changes.
