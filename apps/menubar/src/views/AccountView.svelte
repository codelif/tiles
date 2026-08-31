<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onDestroy } from "svelte";

  import Avatar from "../lib/Avatar.svelte";
  import Hairline from "../lib/Hairline.svelte";
  import Navbar from "../lib/Navbar.svelte";
  import Row from "../lib/Row.svelte";
  import { nav } from "../nav.svelte";
  import { account, truncateDid } from "../state.svelte";

  const COPIED_FOR = 1200;

  // the view is only ever pushed from a local account, but the daemon can go
  // down while it is open
  const local = $derived(account.value.state === "local" ? account.value : null);

  let copied = $state(false);
  let timer = 0;

  onDestroy(() => clearTimeout(timer));

  async function copyDid() {
    if (local === null) return;

    try {
      // the full did, the row only shows it truncated
      await invoke("copy_text", { text: local.did });
    } catch (err) {
      // saying Copied when nothing reached the pasteboard is worse than silence
      console.error("[account]", err);
      return;
    }

    copied = true;
    clearTimeout(timer);
    timer = setTimeout(() => (copied = false), COPIED_FOR);
  }
</script>

<Navbar title="Account" onback={() => nav.pop()} />

<Hairline />

<Row
  title={local?.nickname ?? "No account"}
  sub={local ? truncateDid(local.did) : ""}
  size="large"
  dimmed={local === null}
>
  {#snippet leading()}
    <Avatar nickname={local?.nickname ?? "?"} />
  {/snippet}
</Row>

<Hairline />

<div class="section">
  <Row title="Nickname" dimmed={local === null}>
    {#snippet trailing()}
      <span class="value">{local?.nickname ?? "—"}</span>
    {/snippet}
  </Row>

  <Row title="DID" dimmed={local === null} onselect={local ? copyDid : undefined}>
    {#snippet trailing()}
      {#if copied}
        <span class="value">Copied</span>
      {:else}
        <span class="value value--mono">{local ? truncateDid(local.did) : "—"}</span>
      {/if}
    {/snippet}
  </Row>
</div>

<style>
  .section {
    padding-bottom: 4px;
  }

  .value {
    flex: none;
    max-width: 60%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-small);
    color: var(--text-tertiary);
  }

  .value--mono {
    font-family: var(--font-mono);
  }
</style>
