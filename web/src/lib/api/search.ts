import { api } from "./client";

export interface SearchResult {
  path: string;
  snippet: string;
}

export interface SearchResponse {
  query: string;
  results: SearchResult[];
}

export interface Backlink {
  source: string;
  snippet: string;
}

export function backlinks(projectId: string, path: string) {
  return api<{ path: string; backlinks: Backlink[] }>(
    `/projects/${encodeURIComponent(projectId)}/backlinks?path=${encodeURIComponent(path)}`,
  );
}

export function searchProject(projectId: string, q: string, limit = 10) {
  return api<SearchResponse>(
    `/projects/${encodeURIComponent(projectId)}/search?q=${encodeURIComponent(q)}&limit=${limit}`,
  );
}
