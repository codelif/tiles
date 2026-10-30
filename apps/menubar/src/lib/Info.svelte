<script lang="ts">
  interface Props {
    text: string;
  }

  let { text }: Props = $props();
</script>

<button type="button" class="info" aria-label={text}>
  <svg viewBox="0 0 12 12" width="12" height="12" aria-hidden="true">
    <circle cx="6" cy="6" r="5.2" fill="none" stroke="currentColor" stroke-width="1.2" />
    <circle cx="6" cy="3.7" r="0.75" fill="currentColor" />
    <path d="M6 5.4v3.4" stroke="currentColor" stroke-width="1.2" />
  </svg>
  <span class="info__tip" aria-hidden="true">{text}</span>
</button>

<style>
  /* static, so the tip spans the navbar rather than starting at the icon */
  .info {
    flex: none;
    display: inline-flex;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--slate);
    outline: none;
    transition: color var(--dur-state) ease-out;
  }

  .info:hover,
  .info:focus-visible {
    color: var(--bone);
  }

  .info__tip {
    position: absolute;
    left: var(--pad-x);
    right: var(--pad-x);
    top: 100%;
    z-index: 3;
    padding: 7px 10px;
    clip-path: var(--clip-cut);
    background: var(--steel);
    color: var(--ash);
    font-size: var(--fs-label);
    line-height: 1.35;
    white-space: normal;
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--dur-state) ease-out;
  }

  .info:hover .info__tip,
  .info:focus-visible .info__tip {
    opacity: 1;
  }
</style>
