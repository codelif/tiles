<script lang="ts">
  import { onDestroy } from "svelte";

  import Avatar from "../lib/Avatar.svelte";
  import Chip from "../lib/Chip.svelte";
  import Navbar from "../lib/Navbar.svelte";
  import Row from "../lib/Row.svelte";
  import Zone from "../lib/Zone.svelte";
  import { Copier } from "../lib/copy.svelte";
  import { nav } from "../nav.svelte";
  import { account, truncateMiddle } from "../state.svelte";

  const copier = new Copier();
  onDestroy(() => copier.dispose());

  const local = $derived(account.value.state === "local" ? account.value : null);
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
    title={local ? truncateMiddle(local.did, 24, 8) : "—"}
    dimmed={local === null}
    onselect={local ? () => copier.copy(local.did) : undefined}
  >
    {#snippet trailing()}
      <Chip
        text={copier.copied ? "Copied" : "Copy"}
        tone={copier.copied ? "signal" : "default"}
      />
    {/snippet}
  </Row>
</Zone>
