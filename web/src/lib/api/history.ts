import { api } from "./client";

export interface CommitSummary {
  sha: string;
  message: string;
  author: string;
  date: string;
}

export interface CommitDetail extends CommitSummary {
  files: Array<{ status: string; path: string }>;
}

export interface DiffStat {
  path: string;
  added: number;
  deleted: number;
}

export interface CommitDiffResponse {
  sha: string;
  format: "numstat" | "patch";
  stats: DiffStat[];
  patch?: string;
}

export interface CommitsResponse {
  commits: CommitSummary[];
  has_more?: boolean;
}

export interface FileHistoryResponse {
  path: string;
  commits: CommitSummary[];
}

export function listCommits(projectId: string, limit = 20, offset = 0, query = "") {
  const params = new URLSearchParams({ limit: String(limit), offset: String(offset) });
  if (query) params.set("q", query);
  return api<CommitsResponse>(
    `/projects/${encodeURIComponent(projectId)}/commits?${params.toString()}`,
  );
}

export function getCommit(projectId: string, sha: string) {
  return api<{ commit: CommitDetail }>(
    `/projects/${encodeURIComponent(projectId)}/commits/${sha}`,
  );
}

export function fileHistory(projectId: string, filePath: string) {
  return api<FileHistoryResponse>(
    `/projects/${encodeURIComponent(projectId)}/files/history/${filePath}`,
  );
}

export function getCommitDiff(projectId: string, sha: string, format: "numstat" | "patch" = "numstat") {
  return api<CommitDiffResponse>(
    `/projects/${encodeURIComponent(projectId)}/commits/${sha}/diff?format=${format}`,
  );
}

export function revertCommit(projectId: string, sha: string) {
  return api<{ commit: CommitSummary }>(
    `/projects/${encodeURIComponent(projectId)}/commits/${sha}/revert`,
    { method: "POST", body: JSON.stringify({}) },
  );
}
