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
}

export function listAudit(projectId: string) {
  return api<AuditResponse>(`/projects/${encodeURIComponent(projectId)}/audit`);
}
