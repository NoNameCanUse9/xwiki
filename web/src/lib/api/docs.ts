import { api } from "./client";

export interface TreeEntry {
  name: string;
  type: "blob" | "tree";
  path: string;
}

export interface TreeResponse {
  path: string;
  tree: TreeEntry[];
}

export interface PageResponse {
  path: string;
  format: "raw" | "html";
  content: string;
  revision?: string;
}

export function getTree(projectId: string, dirPath = "") {
  const q = dirPath ? `?path=${encodeURIComponent(dirPath)}` : "";
  return api<TreeResponse>(`/projects/${encodeURIComponent(projectId)}/docs/tree${q}`);
}

export function getPage(
  projectId: string,
  filePath: string,
  format: "raw" | "html" = "html",
  at?: string,
) {
  const q = new URLSearchParams({ format });
  if (at) q.set("at", at);
  return api<PageResponse>(
    `/projects/${encodeURIComponent(projectId)}/docs/pages/${filePath}?${q.toString()}`,
  );
}

export function getHome(projectId: string) {
  return api<PageResponse>(`/projects/${encodeURIComponent(projectId)}/docs/home`);
}
