import { api } from "./client";
import type {
  CreateProjectInput,
  ProjectListResponse,
  ProjectResponse,
} from "./types";

export function listProjects() {
  return api<ProjectListResponse>("/projects");
}

export function getProject(id: string) {
  return api<ProjectResponse>(`/projects/${encodeURIComponent(id)}`);
}

export function createProject(input: CreateProjectInput) {
  return api<ProjectResponse>("/projects", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export function archiveProject(id: string) {
  return api<ProjectResponse>(`/projects/${encodeURIComponent(id)}/archive`, {
    method: "POST",
  });
}
