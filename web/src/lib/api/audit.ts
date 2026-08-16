import { api } from "./client";

export interface AuditEntry {
  id: string;
  actor_type: string;
  actor_id: string;
  project_id?: string;
  action: string;
  path?: string;
  detail?: string;
  request_id?: string;
  created_at: string;
}

export interface AuditResponse {
  entries: AuditEntry[];
  has_more: boolean;
}

export function listAudit(projectId: string, limit = 20, offset = 0) {
  const params = new URLSearchParams({ limit: String(limit), offset: String(offset) });
  return api<AuditResponse>(
    `/projects/${encodeURIComponent(projectId)}/audit?${params.toString()}`,
  );
}
