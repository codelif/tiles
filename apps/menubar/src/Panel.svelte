<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  type Health =
    | { state: "down"; reason: string }
    | { state: "starting" }
    | { state: "up"; version: string };

  const HEALTH_EVENT = "daemon://health";

  let health = $state<Health>({ state: "starting" });

  onMount(() => {
    // the event only fires on a change, so the state at mount has to be asked for
    void invoke<Health>("daemon_health")
      .then((current) => (health = current))
      .catch(() => {});

    const listener = listen<Health>(HEALTH_EVENT, (event) => {
      health = event.payload;
    });
    return () => void listener.then((stop) => stop());
  });

  const label = $derived.by(() => {
    switch (health.state) {
      case "up":
        return `Daemon running · ${health.version}`;
      case "starting":
        return "Starting…";
      case "down":
        return health.reason.charAt(0).toUpperCase() + health.reason.slice(1);
    }
  });
</script>

<div class="panel">
  <div class="status">
    <span class="dot" data-state={health.state}></span>
    <span class="label">{label}</span>
  </div>
</div>

<style>
  .panel {
    width: 100%;
    height: 100%;
    padding: 12px 14px;
    border-radius: var(--radius-panel);
    background: var(--surface);
  }

  .status {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .dot {
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--status-off);
  }

  .dot[data-state="up"] {
    background: var(--status-green);
  }

  .dot[data-state="starting"] {
    background: var(--status-amber);
  }

  .label {
    font-size: var(--fs-status);
    color: var(--text-secondary);
  }
</style>
