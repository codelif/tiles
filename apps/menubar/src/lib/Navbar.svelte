<script lang="ts">
  import Chevron from "./Chevron.svelte";

  let { title, onback }: { title: string; onback: () => void } = $props();
</script>

<div class="navbar">
  <button class="navbar__back" aria-label="Back" onclick={onback}>
    <Chevron back />
  </button>
  <span class="navbar__title">{title}</span>
</div>

<style>
  .navbar {
    display: flex;
    align-items: center;
    gap: 6px;
    height: var(--h-nav-header);
    padding: 0 calc(var(--row-inset) + 4px);
  }

  .navbar__back {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: none;
    border-radius: 8px;
    background: transparent;
  }

  .navbar__back:hover {
    background: var(--row-highlight);
  }

  .navbar__title {
    font-size: var(--fs-row);
    font-weight: var(--fw-semibold);
    color: var(--text-primary);
  }
</style>
