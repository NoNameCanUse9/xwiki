import { api } from "./client";

export interface ShareResponse {
  token: string;
  url: string;
}

/** Create (or reuse) a public share link for a single page. */
export function createShare(projectId: string, path: string) {
  return api<ShareResponse>(
    `/projects/${encodeURIComponent(projectId)}/shares`,
    {
      method: "POST",
      body: JSON.stringify({ path }),
    },
  );
}
