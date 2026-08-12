export interface PatchLine {
	kind: "add" | "delete" | "context";
	content: string;
	oldLine?: number;
	newLine?: number;
}

export interface PatchHunk {
	oldStart: number;
	newStart: number;
	heading: string;
	lines: PatchLine[];
}

export interface FilePatch {
	path: string;
	hunks: PatchHunk[];
}

/** Parse the useful, document-oriented parts of a unified Git patch. */
export function parseDocumentPatch(patch: string): FilePatch[] {
	const files: FilePatch[] = [];
	let file: FilePatch | null = null;
	let hunk: PatchHunk | null = null;
	let oldLine = 0;
	let newLine = 0;

	for (const line of patch.split("\n")) {
		if (line.startsWith("diff --git ")) {
			file = { path: "", hunks: [] };
			files.push(file);
			hunk = null;
			continue;
		}
		if (!file) continue;
		if (line.startsWith("+++ ")) {
			const path = line.slice(4).trim();
			if (path !== "/dev/null") file.path = path.replace(/^b\//, "");
			continue;
		}
		if (!file.path && line.startsWith("--- ")) {
			const path = line.slice(4).trim();
			if (path !== "/dev/null") file.path = path.replace(/^a\//, "");
			continue;
		}
		const header = line.match(
			/^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@\s*(.*)$/,
		);
		if (header) {
			oldLine = Number(header[1]);
			newLine = Number(header[2]);
			hunk = {
				oldStart: oldLine,
				newStart: newLine,
				heading: header[3].trim(),
				lines: [],
			};
			file.hunks.push(hunk);
			continue;
		}
		if (!hunk || line.startsWith("\\ No newline")) continue;

		const marker = line[0];
		const content = line.slice(1);
		const markdownHeading = content.match(/^\s{0,3}#{1,6}\s+(.+?)\s*#*$/);
		if (
			markdownHeading &&
			marker !== "-" &&
			hunk.lines.every((item) => item.kind === "context")
		) {
			hunk.heading = markdownHeading[1];
		}

		if (marker === "+") {
			hunk.lines.push({ kind: "add", content, newLine });
			newLine++;
		} else if (marker === "-") {
			hunk.lines.push({ kind: "delete", content, oldLine });
			oldLine++;
		} else if (marker === " ") {
			hunk.lines.push({ kind: "context", content, oldLine, newLine });
			oldLine++;
			newLine++;
		}
	}

	return files.filter((entry) => entry.path && entry.hunks.length > 0);
}
