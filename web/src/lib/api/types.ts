export interface User {
  id: string;
  username: string;
  display_name: string;
  is_admin: boolean;
}

export interface AuthResponse {
  user: User;
}

export interface Project {
  id: string;
  name: string;
  description: string;
  repo_dir: string;
  archived: boolean;
  archived_at?: string;
  created_at: string;
  updated_at: string;
}

export interface ProjectListResponse {
  projects: Project[];
}

export interface ProjectResponse {
  project: Project;
}

export interface CreateProjectInput {
  name: string;
  description?: string;
}
