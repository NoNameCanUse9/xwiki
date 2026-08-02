import { api } from "./client";

export type ChangeOp = "create" | "update" | "delete" | "move";

export interface ChangeInput {
  op: ChangeOp;
  path: string;
  content?: string;
  encoding?: "base64";
  new_path?: string;
}

export interface ChangesetInput {
  base_revision: string;
  message: string;
  changes: ChangeInput[];
}

export interface ChangesetResult {
  commit: string;
  revision: string;
  preview?: {
    tree: string;
    changes: Array<{ op: string; path: string; status: string }>;
  };
}

export interface RevisionResponse {
  revision: string;
}

export function getRevision(projectId: string) {
  return api<RevisionResponse>(
    `/projects/${encodeURIComponent(projectId)}/revision`,
  );
}

export function submitChangeset(projectId: string, input: ChangesetInput) {
  return api<ChangesetResult>(
    `/projects/${encodeURIComponent(projectId)}/changesets`,
    {
      method: "POST",
      body: JSON.stringify(input),
    },
  );
}
