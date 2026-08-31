import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Health =
  | { state: "down"; reason: string }
  | { state: "starting" }
  | { state: "up"; version: string };

export type Power = "unknown" | "off" | "starting" | "on";
export type Inference = { power: Power; model: string | null };

export type Account =
  | { state: "unknown" }
  | { state: "none" }
  | { state: "local"; did: string; nickname: string };

export type Session = { id: string; name: string; createdAt: number };
export type Sessions = { state: "unknown" } | { state: "ready"; sessions: Session[] };

export const health = $state<{ value: Health }>({ value: { state: "starting" } });
export const inference = $state<{ value: Inference }>({
  value: { power: "unknown", model: null },
});
export const account = $state<{ value: Account }>({ value: { state: "unknown" } });
export const sessions = $state<{ value: Sessions }>({ value: { state: "unknown" } });

/**
 * events only fire on a change, so every state has to be asked for once as
 * well. returns the teardown for all four listeners
 */
export function connect(): () => void {
  const read = <T>(command: string, apply: (value: T) => void) => {
    void invoke<T>(command).then(apply).catch(() => {});
  };

  read<Health>("daemon_health", (v) => (health.value = v));
  read<Inference>("inference_state", (v) => (inference.value = v));
  read<Account>("account_state", (v) => (account.value = v));
  read<Sessions>("sessions_state", (v) => (sessions.value = v));

  const listeners: Promise<UnlistenFn>[] = [
    listen<Health>("daemon://health", (e) => (health.value = e.payload)),
    listen<Inference>("inference://state", (e) => (inference.value = e.payload)),
    listen<Account>("account://state", (e) => (account.value = e.payload)),
    listen<Sessions>("sessions://state", (e) => (sessions.value = e.payload)),
  ];

  return () => listeners.forEach((l) => void l.then((stop) => stop()));
}

/** both ends carry the meaning, the middle is a base58 blur at 380px */
export function truncateDid(did: string): string {
  return did.length <= 24 ? did : `${did.slice(0, 16)}…${did.slice(-6)}`;
}
