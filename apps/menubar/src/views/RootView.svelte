<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  import Avatar from "../lib/Avatar.svelte";
  import Chevron from "../lib/Chevron.svelte";
  import Hairline from "../lib/Hairline.svelte";
  import Row from "../lib/Row.svelte";
  import { nav } from "../nav.svelte";
  import { account, health, inference, truncateDid } from "../state.svelte";

  let pending = $state(false);

  const label = $derived.by(() => {
    switch (health.value.state) {
      case "up":
        return `Daemon running · ${health.value.version}`;
      case "starting":
        return "Starting…";
      case "down":
        return health.value.reason.charAt(0).toUpperCase() + health.value.reason.slice(1);
    }
  });

  // starting counts as on, the switch shows where it is heading
  const on = $derived(inference.value.power === "on" || inference.value.power === "starting");
  const reachable = $derived(inference.value.power !== "unknown");

  // the spec is org-qualified and the org is noise at 380px
  const model = $derived(inference.value.model?.split("/").at(-1) ?? "No model configured");

  const identity = $derived.by(() => {
    switch (account.value.state) {
      case "local":
        return {
          name: account.value.nickname,
          title: account.value.nickname,
          sub: truncateDid(account.value.did),
        };
      case "none":
        return { name: "?", title: "No account yet", sub: "Run tiles account create" };
      // nothing to report rather than nothing to report yet
      case "unknown":
        return { name: "?", title: "—", sub: "" };
    }
  });

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

<div class="status">
  <span class="dot" data-state={health.value.state}></span>
  <span class="status__label">{label}</span>
</div>

<Hairline />

<Row title="Inference" sub={model} size="large" dimmed={!reachable}>
  {#snippet trailing()}
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
  {/snippet}
</Row>

<Hairline />

<Row
  label="Local identity"
  title={identity.title}
  sub={identity.sub}
  size="identity"
  dimmed={account.value.state === "unknown"}
  onselect={account.value.state === "local" ? () => nav.push("account") : undefined}
>
  {#snippet leading()}
    <Avatar nickname={identity.name} />
  {/snippet}
  {#snippet trailing()}
    {#if account.value.state === "local"}<Chevron />{/if}
  {/snippet}
</Row>

<style>
  /* the switch is the one control that has to read as AppKit rather than web */
  .toggle {
    --toggle-w: 54px;
    --toggle-h: 24px;
    --knob-w: 32px;
    --knob-h: 20px;
    --dur-fade: 120ms;

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

  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 2px var(--content-pad-x);
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

  .status__label {
    font-size: var(--fs-status);
    color: var(--text-secondary);
  }

  @media (prefers-reduced-motion: reduce) {
    .toggle,
    .toggle__knob {
      transition: none;
    }
  }
</style>
