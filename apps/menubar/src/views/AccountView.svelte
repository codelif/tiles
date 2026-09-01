<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onDestroy, onMount } from "svelte";

  import Avatar from "../lib/Avatar.svelte";
  import CopyMark from "../lib/CopyMark.svelte";
  import Navbar from "../lib/Navbar.svelte";
  import OpenMark from "../lib/OpenMark.svelte";
  import Row from "../lib/Row.svelte";
  import Zone from "../lib/Zone.svelte";
  import { Copier } from "../lib/copy.svelte";
  import { nav } from "../nav.svelte";
  import { account, truncateMiddle } from "../state.svelte";

  const copier = new Copier();
  onDestroy(() => copier.dispose());

  const local = $derived(account.value.state === "local" ? account.value : null);

  // read once on open rather than polled, the daemon rewrites it only on a
  // `tiles data path` and this view is not on screen for long
  let dataDir = $state<string | null>(null);
  onMount(() => {
    void invoke<string>("data_dir")
      .then((dir) => (dataDir = dir))
      .catch(() => {});
  });

  const home = $derived(dataDir?.replace(/^\/Users\/[^/]+/, "~") ?? "—");
</script>

<Navbar title="Account" onback={() => nav.pop()} />

<!-- no label, an avatar beside a name does not need one told -->
<Zone>
  <Row size="large" title={local?.nickname ?? "—"} sub="Local account" dimmed={local === null}>
    {#snippet leading()}
      <Avatar nickname={local?.nickname ?? "?"} size={26} />
    {/snippet}
  </Row>
</Zone>

<Zone label="Decentralised ID">
  <Row
    mono
    title={local ? truncateMiddle(local.did, 30, 10) : "—"}
    dimmed={local === null}
    onselect={local ? () => copier.copy(local.did) : undefined}
  >
    {#snippet inline()}
      <CopyMark copied={copier.copied} />
    {/snippet}
  </Row>
</Zone>

<Zone label="Data folder">
  <Row
    mono
    title={home}
    dimmed={dataDir === null}
    onselect={dataDir ? () => void invoke("reveal_path", { path: dataDir }).catch(() => {}) : undefined}
  >
    {#snippet inline()}
      {#if dataDir}<OpenMark />{/if}
    {/snippet}
  </Row>
</Zone>
