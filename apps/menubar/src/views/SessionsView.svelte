<script lang="ts">
  import Hairline from "../lib/Hairline.svelte";
  import Navbar from "../lib/Navbar.svelte";
  import Row from "../lib/Row.svelte";
  import SectionLabel from "../lib/SectionLabel.svelte";
  import SessionList from "../lib/SessionList.svelte";
  import { nav } from "../nav.svelte";
  import { sessions } from "../state.svelte";

  // it is only ever pushed past the root preview, but the daemon can go down
  // while it is open
  const all = $derived(sessions.value.state === "ready" ? sessions.value.sessions : []);
</script>

<Navbar title="Sessions" onback={() => nav.pop()} />

<Hairline />

<div class="section">
  {#if all.length > 0}
    <!-- the route caps the list, so this is what is reachable, not what exists -->
    <SectionLabel text="{all.length} most recent" />

    <div class="list">
      <SessionList sessions={all} />
    </div>
  {:else}
    <Row size="large" title="—" dimmed />
  {/if}
</div>

<style>
  .section {
    padding: 2px 0 4px;
  }

  /* the panel clamps to the screen and clips whatever is past it, so the list
     has to run out of room before the window does */
  .list {
    max-height: calc(8 * var(--h-row-lg));
    overflow-y: auto;
  }
</style>
