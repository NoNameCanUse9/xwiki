import { api } from "./client";

export interface ImportFile {
  path: string;
  content: string; // base64
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
