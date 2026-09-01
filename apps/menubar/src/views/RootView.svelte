<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onDestroy } from "svelte";

  import Avatar from "../lib/Avatar.svelte";
  import Chevron from "../lib/Chevron.svelte";
  import Hairline from "../lib/Hairline.svelte";
  import Row from "../lib/Row.svelte";
  import SectionLabel from "../lib/SectionLabel.svelte";
  import SessionList from "../lib/SessionList.svelte";
  import Toggle from "../lib/Toggle.svelte";
  import { Copier } from "../lib/copy.svelte";
  import { nav } from "../nav.svelte";
  import { account, health, inference, remote, sessions, truncateMiddle } from "../state.svelte";

  /** how many fit under the account row before the panel gets tall */
  const PREVIEW = 3;

  let inferencePending = $state(false);
  let sharePending = $state(false);

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
          sub: truncateMiddle(account.value.did, 16, 6),
        };
      case "none":
        return { name: "?", title: "No account yet", sub: "Run tiles account create" };
      // nothing to report rather than nothing to report yet
      case "unknown":
        return { name: "?", title: "—", sub: "" };
    }
  });

  const recent = $derived(sessions.value.state === "ready" ? sessions.value.sessions : []);
  // pushing would show exactly what is already on screen
  const hasMore = $derived(recent.length > PREVIEW);

  // the proxy forwards straight to the local server, so a ticket handed out
  // with inference down answers nothing
  const canShare = $derived(inference.value.power === "on");
  const sharing = $derived(remote.value.state === "sharing" ? remote.value : null);
  const shareSub = $derived.by(() => {
    switch (remote.value.state) {
      // still shared, and the only way to stop it is this row
      case "sharing":
        return canShare ? "Reachable by peers" : "Sharing, inference is off";
      case "off":
        return canShare ? "Off" : "Start inference first";
      case "unknown":
        return "—";
    }
  });

  const copier = new Copier();
  onDestroy(() => copier.dispose());

  async function share() {
    // turning it off has to stay reachable even once inference has gone
    if (sharePending || (!canShare && sharing === null)) return;

    sharePending = true;
    try {
      await invoke("remote_set", { on: sharing === null });
    } catch (err) {
      console.error("[remote]", err);
    } finally {
      sharePending = false;
    }
  }

  async function toggle() {
    if (!reachable || inferencePending) return;

    inferencePending = true;
    try {
      await invoke("inference_set", { on: !on });
    } catch (err) {
      console.error("[inference]", err);
    } finally {
      inferencePending = false;
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
    <Toggle
      {on}
      disabled={!reachable || inferencePending}
      label="Inference server"
      onchange={toggle}
    />
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

<Hairline />

<div class="section">
  <SectionLabel text="Sessions" />

  {#if recent.length > 0}
    <SessionList sessions={recent.slice(0, PREVIEW)} />

    {#if hasMore}
      <Row title="All sessions" accent onselect={() => nav.push("sessions")}>
        {#snippet trailing()}
          <span class="count">{recent.length}</span>
          <Chevron />
        {/snippet}
      </Row>
    {/if}
  {:else if sessions.value.state === "ready"}
    <Row size="large" title="No sessions yet" sub="Start one with tiles run" dimmed />
  {:else}
    <Row size="large" title="—" dimmed />
  {/if}
</div>

<Hairline />

<div class="section">
  <SectionLabel text="Remote inference" />

  <Row
    size="large"
    title="Share inference"
    sub={shareSub}
    dimmed={remote.value.state === "unknown" || (!canShare && sharing === null)}
  >
    {#snippet trailing()}
      <Toggle
        on={sharing !== null}
        disabled={sharePending || (!canShare && sharing === null)}
        label="Share inference"
        onchange={share}
      />
    {/snippet}
  </Row>

  {#if sharing}
    <!-- the full ticket, the row only shows it truncated -->
    <Row title="Ticket" onselect={() => copier.copy(sharing.ticket)}>
      {#snippet trailing()}
        {#if copier.copied}
          <span class="value">Copied</span>
        {:else}
          <span class="value value--mono">{truncateMiddle(sharing.ticket, 10, 6)}</span>
        {/if}
      {/snippet}
    </Row>
  {/if}
</div>

<style>
  .section {
    padding: 2px 0 4px;
  }

  .count,
  .value {
    flex: none;
    font-size: var(--fs-small);
    color: var(--text-tertiary);
  }

  .value {
    max-width: 55%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .value--mono {
    font-family: var(--font-mono);
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
</style>
