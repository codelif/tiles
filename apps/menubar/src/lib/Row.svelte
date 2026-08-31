<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    title?: string;
    sub?: string;
    /** eyebrow above the title, three-line rows only */
    label?: string;
    size?: "regular" | "large" | "identity";
    dimmed?: boolean;
    leading?: Snippet;
    trailing?: Snippet;
    onselect?: () => void;
  }

  let {
    title,
    sub,
    label,
    size = "regular",
    dimmed = false,
    leading,
    trailing,
    onselect,
  }: Props = $props();
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="row"
  data-size={size}
  data-dimmed={dimmed}
  data-selectable={onselect !== undefined}
  role={onselect ? "menuitem" : undefined}
  onclick={onselect}
>
  {@render leading?.()}

  <div class="row__main">
    {#if label}<span class="row__label">{label}</span>{/if}
    {#if title}<span class="row__title">{title}</span>{/if}
    {#if sub}<span class="row__sub">{sub}</span>{/if}
  </div>

  {@render trailing?.()}
</div>

<style>
  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: var(--h-row);
    padding: 0 calc(var(--row-inset) + var(--row-pad-x));
  }

  .row[data-size="large"] {
    min-height: var(--h-row-lg);
  }

  .row[data-size="identity"] {
    min-height: var(--h-identity);
  }

  .row[data-dimmed="true"] {
    opacity: 0.4;
  }

  /* the highlight sits behind the content, inset from the panel edge */
  .row::before {
    content: "";
    position: absolute;
    inset: var(--row-inset-y) var(--row-inset);
    border-radius: var(--radius-row);
    background: transparent;
  }

  .row[data-selectable="true"][data-dimmed="false"]:hover::before {
    background: var(--row-highlight);
  }

  /* positioned, so they paint above ::before */
  .row > :global(*) {
    position: relative;
  }

  .row__main {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 1px;
    min-width: 0;
    flex: 1;
  }

  .row__label,
  .row__title,
  .row__sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row__label {
    font-size: var(--fs-small);
    color: var(--text-tertiary);
  }

  .row__sub {
    font-size: var(--fs-small);
    color: var(--text-tertiary);
  }
</style>
