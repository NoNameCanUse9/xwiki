import { api } from "./client";

export interface SearchResult {
  path: string;
  snippet: string;
}

export interface SearchResponse {
  query: string;
  results: SearchResult[];
}

export function searchProject(projectId: string, q: string, limit = 10) {
  return api<SearchResponse>(
    `/projects/${encodeURIComponent(projectId)}/search?q=${encodeURIComponent(q)}&limit=${limit}`,
  );
}
