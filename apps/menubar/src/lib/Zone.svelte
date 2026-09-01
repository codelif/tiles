<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    label?: string;
    children: Snippet;
  }

  let { label, children }: Props = $props();
</script>

<section class="zone">
  <!-- full bleed, an inset separator is the macos idiom this pass drops -->
  <div class="zone__rule"></div>
  {#if label}<h2 class="zone__label">{label}</h2>{/if}
  {@render children()}
</section>

<style>
  .zone {
    padding-bottom: 7px;
  }

  .zone__rule {
    height: var(--hairline);
    margin-bottom: 9px;
    background: var(--rule);
  }

  .zone__label {
    padding: 0 var(--pad-x) 5px;
    font-size: var(--fs-label);
    font-weight: 500;
    color: var(--slate);
  }
</style>
