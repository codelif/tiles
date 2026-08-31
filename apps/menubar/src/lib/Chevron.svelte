<script lang="ts">
  let { back = false }: { back?: boolean } = $props();
</script>

<svg class="chevron" data-back={back} viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
  <path
    d="M6 3.5 10.5 8 6 12.5"
    fill="none"
    stroke="currentColor"
    stroke-width="1.6"
    stroke-linecap="round"
    stroke-linejoin="round"
  />
</svg>

<style>
  .chevron {
    flex: none;
    color: var(--text-tertiary);
  }

  .chevron[data-back="true"] {
    transform: rotate(180deg);
    color: var(--text-secondary);
  }
</style>
