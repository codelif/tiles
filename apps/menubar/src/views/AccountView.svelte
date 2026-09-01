<script lang="ts">
  import { onDestroy } from "svelte";

  import Avatar from "../lib/Avatar.svelte";
  import Hairline from "../lib/Hairline.svelte";
  import Navbar from "../lib/Navbar.svelte";
  import Row from "../lib/Row.svelte";
  import { Copier } from "../lib/copy.svelte";
  import { nav } from "../nav.svelte";
  import { account, truncateMiddle } from "../state.svelte";

  const shortDid = (did: string) => truncateMiddle(did, 16, 6);

  // the view is only ever pushed from a local account, but the daemon can go
  // down while it is open
  const local = $derived(account.value.state === "local" ? account.value : null);

  const copier = new Copier();
  onDestroy(() => copier.dispose());
</script>

<Navbar title="Account" onback={() => nav.pop()} />

<Hairline />

<Row
  title={local?.nickname ?? "No account"}
  sub={local ? shortDid(local.did) : ""}
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

  <!-- the full did, the row only shows it truncated -->
  <Row
    title="DID"
    dimmed={local === null}
    onselect={local ? () => copier.copy(local.did) : undefined}
  >
    {#snippet trailing()}
      {#if copier.copied}
        <span class="value">Copied</span>
      {:else}
        <span class="value value--mono">{local ? shortDid(local.did) : "—"}</span>
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
