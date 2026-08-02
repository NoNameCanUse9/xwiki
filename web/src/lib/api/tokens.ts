import { api } from "./client";

export interface AgentToken {
  id: string;
  name: string;
  scope: "read" | "write";
  project_ids: string[];
  path_prefixes: string[];
  created_at: string;
  revoked_at?: string;
}

export interface CreateTokenInput {
  name: string;
  scope: "read" | "write";
  project_ids: string[];
  path_prefixes: string[];
}

export interface CreatedTokenResponse {
  token: AgentToken;
  secret: string;
}

export function listTokens() {
  return api<{ tokens: AgentToken[] }>("/tokens");
}

export function createToken(input: CreateTokenInput) {
  return api<CreatedTokenResponse>("/tokens", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export function revokeToken(id: string) {
  return api<{ ok: boolean }>(`/tokens/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
}
