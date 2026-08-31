<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  type Health =
    | { state: "down"; reason: string }
    | { state: "starting" }
    | { state: "up"; version: string };

  type Power = "unknown" | "off" | "starting" | "on";
  type Inference = { power: Power; model: string | null };

  const HEALTH_EVENT = "daemon://health";
  const INFERENCE_EVENT = "inference://state";

  let health = $state<Health>({ state: "starting" });
  let inference = $state<Inference>({ power: "unknown", model: null });
  let pending = $state(false);

  onMount(() => {
    // events only fire on a change, so the state at mount has to be asked for
    void invoke<Health>("daemon_health")
      .then((current) => (health = current))
      .catch(() => {});
    void invoke<Inference>("inference_state")
      .then((current) => (inference = current))
      .catch(() => {});

    const listeners = [
      listen<Health>(HEALTH_EVENT, (event) => (health = event.payload)),
      listen<Inference>(INFERENCE_EVENT, (event) => (inference = event.payload)),
    ];
    return () => listeners.forEach((l) => void l.then((stop) => stop()));
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

  // starting counts as on, the switch shows where it is heading
  const on = $derived(inference.power === "on" || inference.power === "starting");
  const reachable = $derived(inference.power !== "unknown");

  // the spec is org-qualified and the org is noise at 380px
  const model = $derived(
    inference.model?.split("/").at(-1) ?? "No model configured",
  );

  async function toggle() {
    if (!reachable || pending) return;

    pending = true;
    try {
      await invoke("inference_set", { on: !on });
    } catch (err) {
      console.error("[inference]", err);
    } finally {
      pending = false;
    }
  }
</script>

<div class="panel">
  <div class="status">
    <span class="dot" data-state={health.state}></span>
    <span class="label">{label}</span>
  </div>

  <div class="hairline"></div>

  <div class="row" data-disabled={!reachable}>
    <div class="row__main">
      <span class="row__title">Inference</span>
      <span class="row__sub">{model}</span>
    </div>

    <button
      class="toggle"
      role="switch"
      aria-checked={on}
      aria-label="Inference server"
      data-on={on}
      disabled={!reachable || pending}
      onclick={toggle}
    >
      <span class="toggle__knob"></span>
    </button>
  </div>
</div>

<style>
  /* the switch is the one control that has to read as AppKit rather than web */
  .panel {
    --toggle-w: 54px;
    --toggle-h: 24px;
    --knob-w: 32px;
    --knob-h: 20px;
    --dur-fade: 120ms;
    --ease-push: cubic-bezier(0.32, 0.72, 0, 1);

    width: 100%;
    height: 100%;
    padding: 10px 0;
    border-radius: var(--radius-panel);
    background: var(--surface);
  }

  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 2px 14px;
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

  .hairline {
    height: 0.5px;
    margin: 8px 14px;
    background: var(--separator);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 32px;
    padding: 0 14px;
  }

  .row[data-disabled="true"] {
    opacity: 0.4;
  }

  .row__main {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    flex: 1;
  }

  .row__title,
  .row__sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row__sub {
    font-size: var(--fs-small);
    color: var(--text-tertiary);
  }

  .toggle {
    position: relative;
    flex: none;
    width: var(--toggle-w);
    height: var(--toggle-h);
    border: none;
    border-radius: calc(var(--toggle-h) / 2);
    background: var(--track-off);
    box-shadow: inset 0 0 0 0.5px var(--track-rim);
    transition: background var(--dur-fade) ease-out;
  }

  .toggle[data-on="true"] {
    background: var(--accent);
    box-shadow: inset 0 0 0 0.5px rgba(0, 0, 0, 0.06);
  }

  .toggle__knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: var(--knob-w);
    height: var(--knob-h);
    border-radius: calc(var(--knob-h) / 2);
    background: var(--knob-off);
    box-shadow: var(--knob-shadow);
    transition: transform var(--dur-fade) var(--ease-push);
  }

  .toggle[data-on="true"] .toggle__knob {
    transform: translateX(calc(var(--toggle-w) - var(--knob-w) - 4px));
    background: var(--knob);
  }

  @media (prefers-reduced-motion: reduce) {
    .toggle,
    .toggle__knob {
      transition: none;
    }
  }
</style>
