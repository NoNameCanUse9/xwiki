import { api } from "./client";

export interface ImportFile {
	path: string;
	content: string; // base64
}

export function importRepo(name: string, url: string) {
	return api<{ project: { id: string; name: string }; commits: number }>(
		`/import/repo?name=${encodeURIComponent(name)}&url=${encodeURIComponent(url)}`,
		{ method: "POST", body: "{}" },
	);
}

export function importZip(
	projectId: string,
	baseRevision: string,
	files: ImportFile[],
	message = "Import zip snapshot",
) {
	return api<{ commit: string; revision: string; imported: number }>(
		`/projects/${encodeURIComponent(projectId)}/import`,
		{
			method: "POST",
			body: JSON.stringify({ base_revision: baseRevision, message, files }),
		},
	);
}

/** Import a folder of files as a new project (multipart/form-data). */
export async function importFolder(
	name: string,
	description: string,
	files: File[],
): Promise<{ project: { id: string; name: string }; commits: number }> {
	const form = new FormData();
	form.set("name", name);
	form.set("description", description);
	for (const f of files) {
		// zip 解压模式：__relPath 已是最终相对路径，直接使用；
		// 文件夹模式：webkitRelativePath 是 "folder/sub/file.txt"，剥离首层目录。
		const rel =
			(f as File & { __relPath?: string }).__relPath ??
			((f.webkitRelativePath ?? f.name).split("/").slice(1).join("/") ||
				f.name);
		form.append("paths", rel);
		form.append("files", f, rel);
	}
	const res = await fetch("/api/v1/projects/import-folder", {
		method: "POST",
		credentials: "include",
		body: form,
	});
	if (!res.ok) {
		let code = "internal_error";
		let message = `\u8BF7\u6C42\u5931\u8D25\uFF08HTTP ${res.status}\uFF09`;
		try {
			const body = (await res.json()) as {
				error?: { code?: string; message?: string };
			};
			code = body.error?.code ?? code;
			message = body.error?.message ?? message;
		} catch {
			// non-JSON
		}
		const { ApiError } = await import("./client");
		throw new ApiError(res.status, code, message);
	}
	return (await res.json()) as {
		project: { id: string; name: string };
		commits: number;
	};
}
