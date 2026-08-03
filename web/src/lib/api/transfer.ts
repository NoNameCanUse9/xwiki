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
		// webkitRelativePath gives "folder/sub/file.txt" — strip leading dir.
		// Go's multipart parser strips directory parts from filename=, so send
		// the real relative path separately, index-aligned with files.
		const rel = (f.webkitRelativePath ?? f.name).split("/").slice(1).join("/");
		form.append("paths", rel || f.name);
		form.append("files", f, rel || f.name);
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
