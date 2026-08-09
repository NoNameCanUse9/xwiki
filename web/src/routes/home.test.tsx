import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";
import HomePage from "./home";
import * as projectsApi from "@/lib/api/projects";
import * as authStore from "@/stores/auth";
import type { Project } from "@/lib/api/types";

vi.mock("@/lib/api/projects", () => ({
	listProjects: vi.fn(),
	archiveProject: vi.fn(),
	createProject: vi.fn(),
	unarchiveProject: vi.fn(),
	renameProject: vi.fn(),
	deleteProject: vi.fn(),
}));

const sampleProject = (over: Partial<Project> = {}): Project => ({
	id: "prj_1",
	name: "docs-site",
	description: "产品文档",
	repo_dir: "repos/prj_1/repo.git",
	archived: false,
	created_at: "2026-08-02T12:00:00Z",
	updated_at: "2026-08-02T12:00:00Z",
	...over,
});

function renderPage() {
	const qc = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	return render(
		<QueryClientProvider client={qc}>
			<MemoryRouter>
				<HomePage />
				<Toaster />
			</MemoryRouter>
		</QueryClientProvider>,
	);
}

describe("HomePage", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		authStore.useAuthStore.setState({
			user: {
				id: "usr_1",
				username: "admin",
				display_name: "Admin",
				is_admin: true,
			},
			initializing: false,
			login: vi.fn(),
			logout: vi.fn(),
			fetchMe: vi.fn(),
		});
	});

	it("shows the empty state when there are no projects", async () => {
		vi.mocked(projectsApi.listProjects).mockResolvedValue({ projects: [] });
		renderPage();
		expect(await screen.findByText("还没有项目")).toBeInTheDocument();
	});

	it("renders active and archived project sections", async () => {
		vi.mocked(projectsApi.listProjects).mockResolvedValue({
			projects: [
				sampleProject({ id: "prj_1", name: "docs-site" }),
				sampleProject({ id: "prj_2", name: "legacy", archived: true }),
			],
		});
		renderPage();
		expect(await screen.findAllByText("docs-site")).toHaveLength(2); // sidebar + card
		expect(screen.getAllByText("legacy").length).toBeGreaterThan(0);
		expect(screen.getByText(/active · 1/)).toBeInTheDocument();
		expect(screen.getByText(/archived · 1/)).toBeInTheDocument();
		const repos = screen.getAllByText("repos/prj_1/repo.git");
		expect(repos).toHaveLength(2);
		for (const repo of repos) {
			expect(repo).toHaveClass("min-w-0", "truncate");
			expect(repo.parentElement).toHaveClass("min-w-0");
			expect(repo.parentElement?.lastElementChild).toHaveClass("shrink-0");
		}
	});

	it("archives a project from its action menu and refreshes the list", async () => {
		vi.mocked(projectsApi.listProjects).mockResolvedValue({
			projects: [sampleProject()],
		});
		vi.mocked(projectsApi.archiveProject).mockResolvedValue({
			project: sampleProject({ archived: true }),
		});
		const user = userEvent.setup();
		renderPage();
		await user.click(
			await screen.findByRole("button", { name: "项目操作 docs-site" }),
		);
		await user.click(screen.getByRole("menuitem", { name: "归档" }));
		expect(projectsApi.archiveProject).toHaveBeenCalledWith("prj_1");
	});

	it("opens project actions, shows archived-specific items, and restores", async () => {
		vi.mocked(projectsApi.listProjects).mockResolvedValue({
			projects: [sampleProject({ archived: true })],
		});
		vi.mocked(projectsApi.unarchiveProject).mockResolvedValue({
			project: sampleProject({ archived: false }),
		});
		const user = userEvent.setup();
		renderPage();
		await user.click(
			await screen.findByRole("button", { name: "项目操作 docs-site" }),
		);
		expect(screen.getByRole("menuitem", { name: "重命名" })).toBeInTheDocument();
		expect(screen.getByRole("menuitem", { name: "删除" })).toBeInTheDocument();
		expect(screen.getByRole("menuitem", { name: "恢复" })).toBeInTheDocument();
		expect(screen.queryByRole("menuitem", { name: "归档" })).not.toBeInTheDocument();
		await user.click(screen.getByRole("menuitem", { name: "恢复" }));
		expect(projectsApi.unarchiveProject).toHaveBeenCalledWith("prj_1");
	});

	it("renames a project from its action menu", async () => {
		vi.mocked(projectsApi.listProjects).mockResolvedValue({
			projects: [sampleProject()],
		});
		vi.mocked(projectsApi.renameProject).mockResolvedValue({
			project: sampleProject({ name: "renamed" }),
		});
		const user = userEvent.setup();
		renderPage();
		await user.click(
			await screen.findByRole("button", { name: "项目操作 docs-site" }),
		);
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

	it("confirms before deleting a project with an in-page dialog", async () => {
		vi.mocked(projectsApi.listProjects).mockResolvedValue({
			projects: [sampleProject()],
		});
		vi.mocked(projectsApi.deleteProject).mockResolvedValue({ deleted: true });
		const user = userEvent.setup();
		renderPage();
		await user.click(
			await screen.findByRole("button", { name: "项目操作 docs-site" }),
		);
		await user.click(screen.getByRole("menuitem", { name: "删除" }));
		expect(screen.getByText(/确认删除项目/)).toBeInTheDocument();
		await user.click(screen.getByRole("button", { name: "确认删除" }));
		await vi.waitFor(() =>
			expect(projectsApi.deleteProject).toHaveBeenCalledWith("prj_1"),
		);
	});

	it("does not navigate when clicking menu items on a project card", async () => {
		vi.mocked(projectsApi.listProjects).mockResolvedValue({
			projects: [sampleProject()],
		});
		vi.mocked(projectsApi.archiveProject).mockResolvedValue({
			project: sampleProject({ archived: true }),
		});
		const user = userEvent.setup();
		const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
		render(
			<QueryClientProvider client={qc}>
				<MemoryRouter initialEntries={["/"]}>
					<Routes>
						<Route path="/" element={<HomePage />} />
						<Route path="/projects/:id/docs" element={<div>docs-view</div>} />
					</Routes>
				</MemoryRouter>
				<Toaster />
			</QueryClientProvider>,
		);
		await user.click(
			await screen.findByRole("button", { name: "项目操作 docs-site" }),
		);
		await user.click(screen.getByRole("menuitem", { name: "归档" }));
		await vi.waitFor(() =>
			expect(projectsApi.archiveProject).toHaveBeenCalledWith("prj_1"),
		);
		expect(screen.queryByText("docs-view")).not.toBeInTheDocument();
		expect(screen.getByText("项目")).toBeInTheDocument();
	});

	it("filters projects by status", async () => {
		vi.mocked(projectsApi.listProjects).mockResolvedValue({
			projects: [
				sampleProject({ id: "prj_active", name: "active-project" }),
				sampleProject({
					id: "prj_archived",
					name: "archived-project",
					archived: true,
				}),
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

	it("shows a no-match state when the filter has no results", async () => {
		vi.mocked(projectsApi.listProjects).mockResolvedValue({
			projects: [sampleProject()],
		});
		const user = userEvent.setup();
		renderPage();

		const filter = await screen.findByRole("combobox", { name: "项目状态" });
		await user.selectOptions(filter, "archived");
		expect(screen.getByText("暂无符合条件的项目")).toBeInTheDocument();
		expect(screen.queryByText("还没有项目")).not.toBeInTheDocument();
	});

	it("opens the create dialog and submits a new project", async () => {
		vi.mocked(projectsApi.listProjects).mockResolvedValue({ projects: [] });
		vi.mocked(projectsApi.createProject).mockResolvedValue({
			project: sampleProject(),
		});
		const user = userEvent.setup();
		renderPage();
		await user.click(screen.getByRole("button", { name: "新建项目" }));
		await user.type(await screen.findByLabelText("项目名"), "docs-site");
		await user.click(screen.getByRole("button", { name: "创建" }));
		expect(projectsApi.createProject).toHaveBeenCalledWith({
			name: "docs-site",
			description: "",
		});
	});
});
