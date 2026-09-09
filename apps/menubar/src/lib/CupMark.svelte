<script lang="ts">
  interface Props {
    active: boolean;
  }

  let { active }: Props = $props();
</script>

<svg
  class="cup"
  viewBox="0 0 12 12"
  width="15"
  height="15"
  aria-hidden="true"
  fill="none"
  stroke="currentColor"
  stroke-width="1"
>
  <path d="M1.8 3h6.4l-.9 6h-4.6z" />
  <path d="M8.1 4.4h.9a1.4 1.4 0 0 1 0 2.8h-1" />
  {#if active}
    <path d="M4.3 .5v1.6" />
    <path d="M6.6 .3v1.8" />
  {/if}
</svg>

<style>
  .cup {
    display: block;
    color: inherit;
  }
</style>
