import { api, ApiError } from "./client";

export interface EditLock {
  path: string;
  user_id: string;
  username: string;
  acquired_at: string;
  expires_at: string;
}

interface LockEnvelope {
  lock: EditLock | null;
}

interface ReleaseEnvelope {
  released: boolean;
}

const q = (path: string) =>
  `?path=${encodeURIComponent(path)}`;

/** Current lock state for a page (null when free). */
export function getLock(projectId: string, path: string) {
  return api<LockEnvelope>(`/projects/${encodeURIComponent(projectId)}/locks${q(path)}`);
}

/** Try to take the exclusive edit lock; rejects with 409 page_locked when held. */
export function acquireLock(projectId: string, path: string) {
  return api<LockEnvelope>(
    `/projects/${encodeURIComponent(projectId)}/locks${q(path)}`,
    { method: "POST" },
  );
}

/** Release the lock (holder only; releasing a free lock is a no-op). */
export function releaseLock(projectId: string, path: string) {
  return api<ReleaseEnvelope>(
    `/projects/${encodeURIComponent(projectId)}/locks${q(path)}`,
    { method: "DELETE" },
  );
}

/** Renew the lease; rejects 409 lock_lost when the lock no longer exists. */
export function heartbeatLock(projectId: string, path: string) {
  return api<LockEnvelope>(
    `/projects/${encodeURIComponent(projectId)}/locks/heartbeat${q(path)}`,
    { method: "POST" },
  );
}

/** Force a lock open (any signed-in user; the holder's draft is discarded). */
export function forceReleaseLock(projectId: string, path: string) {
  return api<ReleaseEnvelope>(
    `/projects/${encodeURIComponent(projectId)}/locks/force-release${q(path)}`,
    { method: "POST" },
  );
}

/** The lock info carried by a 409 page_locked ApiError, if present. */
export function lockFromError(err: unknown): EditLock | null {
  if (err instanceof ApiError && err.code === "page_locked") {
    const data = err.data as { lock?: EditLock } | undefined;
    return data?.lock ?? null;
  }
  return null;
}
