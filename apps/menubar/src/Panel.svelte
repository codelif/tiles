<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  import Stack from "./lib/Stack.svelte";
  import { nav, type ViewId } from "./nav.svelte";
  import { connect } from "./state.svelte";
  import AccountView from "./views/AccountView.svelte";
  import RootView from "./views/RootView.svelte";

  let root: HTMLDivElement;

  onMount(() => {
    const disconnect = connect();

    // the stack animates its height, so this fires every frame of a push. one
    // call per frame, and the host ignores a height it already has
    let frame = 0;
    let sent = -1;
    const report = () => {
      frame = 0;
      const height = Math.ceil(root.getBoundingClientRect().height);
      if (height === 0 || height === sent) return;
      sent = height;
      void invoke("resize_panel", { height }).catch(() => {});
    };

    const observer = new ResizeObserver(() => {
      if (frame === 0) frame = requestAnimationFrame(report);
    });
    observer.observe(root);

    return () => {
      disconnect();
      observer.disconnect();
      if (frame !== 0) cancelAnimationFrame(frame);
    };
  });

  // keydown not keyup, the panel should be gone before the key comes back up
  function onkeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.preventDefault();

    if (nav.depth > 0) {
      nav.pop();
    } else {
      void invoke("hide_panel").catch(() => {});
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="panel" bind:this={root}>
  <Stack>
    {#snippet view(id: ViewId)}
      {#if id === "account"}
        <AccountView />
      {:else}
        <RootView />
      {/if}
    {/snippet}
  </Stack>
</div>

<style>
  .panel {
    width: 100%;
    padding: 10px 0;
    border-radius: var(--radius-panel);
    background: var(--surface);
  }
</style>
