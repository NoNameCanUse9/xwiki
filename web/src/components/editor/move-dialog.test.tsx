import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";
import MoveDialog from "./move-dialog";
import * as changesetsApi from "@/lib/api/changesets";
import * as docsApi from "@/lib/api/docs";
import { ApiError } from "@/lib/api/client";

vi.mock("@/lib/api/changesets", () => ({
	getRevision: vi.fn(),
	submitChangeset: vi.fn(),
}));

vi.mock("@/lib/api/docs", () => ({
	getTree: vi.fn(),
}));

function wrap(props: Partial<React.ComponentProps<typeof MoveDialog>> = {}) {
	return render(
		<QueryClientProvider client={new QueryClient()}>
			<MoveDialog
				projectId="prj_1"
				source="docs/guide.md"
				isDir={false}
				open
				onOpenChange={() => {}}
				onDone={() => {}}
				{...props}
			/>
			<Toaster />
		</QueryClientProvider>,
	);
}

beforeEach(() => {
	vi.mocked(docsApi.getTree).mockReset();
	vi.mocked(changesetsApi.getRevision).mockReset();
	vi.mocked(changesetsApi.submitChangeset).mockReset();
	vi.mocked(docsApi.getTree).mockImplementation(async (_projectId, dir) => {
		if (dir === "docs") {
			return {
				path: "docs",
				tree: [{ name: "guide.md", type: "blob", path: "docs/guide.md" }],
			};
		}
		return {
			path: "",
			tree: [
				{ name: "docs", type: "tree", path: "docs" },
				{ name: "archive", type: "tree", path: "archive" },
				{ name: "guide.md", type: "blob", path: "guide.md" },
			],
		};
	});
	vi.mocked(changesetsApi.getRevision).mockResolvedValue({ revision: "r1" });
});

describe("MoveDialog", () => {
	it("loads the root tree and pre-selects the source parent", async () => {
		wrap();
		expect(await screen.findByText("docs")).toBeTruthy();
		expect(screen.getByText("archive")).toBeTruthy();
		// Source parent (docs) is selected by default.
		expect(screen.getByRole("button", { name: /docs/ })).toBeTruthy();
		// Default name prefilled.
		expect(screen.getByLabelText("新名称")).toHaveValue("guide.md");
	});

	it("submits a real move only after the dry run passes", async () => {
		vi.mocked(changesetsApi.submitChangeset).mockResolvedValue({
			commit: "",
			revision: "r1",
			preview: {
				tree: "t",
				changes: [{ op: "move", path: "docs/guide.md", status: "moved" }],
			},
		});
		const user = userEvent.setup();
		wrap();
		await screen.findByText("docs");
		// Select archive as the target folder.
		await user.click(screen.getByText("archive"));
		await user.clear(screen.getByLabelText("新名称"));
		await user.type(screen.getByLabelText("新名称"), "handbook.md");
		await user.click(screen.getByRole("button", { name: "移动" }));

		expect(changesetsApi.submitChangeset).toHaveBeenCalledTimes(2);
		const [dryRunCall, realCall] = vi.mocked(
			changesetsApi.submitChangeset,
		).mock.calls;
		expect(dryRunCall[0]).toBe("prj_1");
		expect(dryRunCall[1]).toEqual({
			base_revision: "r1",
			message: "",
			changes: [{ op: "move", path: "docs/guide.md", new_path: "archive/handbook.md" }],
		});
		expect(dryRunCall[2]).toBe(true);
		expect(realCall[2]).toBeUndefined(); // default: dryRun = false
	});

	it("shows a Chinese conflict toast and skips the real submission on 409", async () => {
		vi.mocked(changesetsApi.submitChangeset).mockRejectedValue(
			new ApiError(409, "path_exists", "target path already exists"),
		);
		const user = userEvent.setup();
		wrap();
		await screen.findByText("docs");
		// Pick a different target folder so the move actually runs.
		await user.click(screen.getByText("archive"));
		await user.click(screen.getByRole("button", { name: "移动" }));

		expect(await screen.findByText(/已存在/)).toBeTruthy();
		expect(changesetsApi.submitChangeset).toHaveBeenCalledTimes(1);
		expect(changesetsApi.submitChangeset).toHaveBeenCalledWith(
			"prj_1",
			expect.anything(),
			true,
		);
	});
});
