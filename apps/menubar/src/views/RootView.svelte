<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onDestroy } from "svelte";

  import Avatar from "../lib/Avatar.svelte";
  import Chevron from "../lib/Chevron.svelte";
  import Chip from "../lib/Chip.svelte";
  import Masthead, { type Mode } from "../lib/Masthead.svelte";
  import ProviderMark from "../lib/ProviderMark.svelte";
  import Row from "../lib/Row.svelte";
  import Switch from "../lib/Switch.svelte";
  import SessionList from "../lib/SessionList.svelte";
  import Zone from "../lib/Zone.svelte";
  import { Copier } from "../lib/copy.svelte";
  import { contextLabel, describe } from "../lib/model";
  import { nav } from "../nav.svelte";
  import { account, health, inference, remote, sessions, truncateMiddle } from "../state.svelte";

  /** how many fit under the masthead before the panel gets tall */
  const PREVIEW = 3;

  const copier = new Copier();
  onDestroy(() => copier.dispose());

  let inferencePending = $state(false);
  let sharePending = $state(false);

  const power = $derived(inference.value.power);
  const on = $derived(power === "on" || power === "starting");

  const mode = $derived.by<Mode>(() => {
    if (health.value.state === "down") return "down";
    if (health.value.state === "starting") return "connecting";
    if (power === "starting") return "starting";
    return power === "on" ? "running" : "idle";
  });

  const status = $derived.by(() => {
    if (health.value.state === "down") return health.value.reason;
    if (health.value.state === "starting") return "Connecting";

    const version = health.value.version;
    if (power === "starting") return `Starting · ${version}`;
    return power === "on" ? `Running · ${version}` : `Idle · ${version}`;
  });

  const model = $derived(inference.value.model ? describe(inference.value.model) : null);
  const modelSub = $derived.by(() => {
    const parts: string[] = [];
    const context = inference.value.llama?.contextLength;
    if (context) parts.push(`${contextLabel(context)} context`);
    if (model?.format) parts.push(model.format);

    return parts.join(" · ");
  });

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
      case "unknown":
        return { name: "?", title: "—", sub: "" };
    }
  });

  const recent = $derived(sessions.value.state === "ready" ? sessions.value.sessions : []);
  // pushing would show exactly what is already on screen
  const hasMore = $derived(recent.length > PREVIEW);

  // the proxy forwards straight to the local server, so a ticket handed out
  // with inference down answers nothing
  const canShare = $derived(power === "on");
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

  async function toggle() {
    if (inferencePending || health.value.state !== "up") return;
    inferencePending = true;
    try {
      await invoke("inference_set", { on: !on });
    } catch (err) {
      console.error("[inference]", err);
    } finally {
      inferencePending = false;
    }
  }

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
</script>

<Masthead
  {mode}
  {status}
  {on}
  disabled={health.value.state !== "up"}
  ontoggle={toggle}
/>

<Zone label="Model">
  {#if model}
    <Row
      size="large"
      title={model.name}
      sub={modelSub}
      submono
      onselect={() => nav.push("model")}
    >
      {#snippet leading()}
        <ProviderMark provider={model.provider} />
      {/snippet}
      {#snippet trailing()}
        {#if model.quant}<Chip text={model.quant} />{/if}
        <Chevron />
      {/snippet}
    </Row>
  {:else}
    <Row size="large" title="No model configured" sub="Run tiles model use" dimmed>
      {#snippet leading()}
        <ProviderMark provider="generic" />
      {/snippet}
    </Row>
  {/if}
</Zone>

<Zone label="Sessions">
  {#if recent.length > 0}
    <SessionList sessions={recent.slice(0, PREVIEW)} />
    {#if hasMore}
      <Row title="All sessions" tone="signal" onselect={() => nav.push("sessions")}>
        {#snippet trailing()}
          <Chip text={String(recent.length)} />
          <Chevron />
        {/snippet}
      </Row>
    {/if}
  {:else}
    <Row title={sessions.value.state === "ready" ? "No chats yet" : "—"} dimmed />
  {/if}
</Zone>

<Zone label="Account">
  <Row
    size="large"
    title={identity.title}
    sub={identity.sub}
    submono={account.value.state === "local"}
    dimmed={account.value.state !== "local"}
    onselect={account.value.state === "local" ? () => nav.push("account") : undefined}
  >
    {#snippet leading()}
      <Avatar nickname={identity.name} />
    {/snippet}
    {#snippet trailing()}
      {#if account.value.state === "local"}<Chevron />{/if}
    {/snippet}
  </Row>
</Zone>

<Zone label="Remote">
  <Row title="Share inference" sub={shareSub} dimmed={remote.value.state === "unknown"}>
    {#snippet trailing()}
      <Switch
        on={sharing !== null}
        disabled={!canShare && sharing === null}
        pending={sharePending}
        label="Share inference"
        onchange={share}
      />
    {/snippet}
  </Row>
  {#if sharing}
    <Row
      key="Ticket"
      mono
      title={truncateMiddle(sharing.ticket, 14, 7)}
      onselect={() => copier.copy(sharing.ticket)}
    >
      {#snippet trailing()}
        <Chip text={copier.copied ? "Copied" : "Copy"} tone={copier.copied ? "signal" : "default"} />
      {/snippet}
    </Row>
  {/if}
</Zone>
