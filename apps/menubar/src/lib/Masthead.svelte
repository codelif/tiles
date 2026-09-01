<script module lang="ts">
  /** running is the only mode that floods, so it is the only one read at a glance */
  export type Mode = "down" | "connecting" | "starting" | "idle" | "running";
</script>

<script lang="ts">
  import Chip from "./Chip.svelte";
  import LlamaCpp from "./LlamaCpp.svelte";
  import Mark from "./Mark.svelte";
  import Switch from "./Switch.svelte";

  interface Props {
    mode: Mode;
    status: string;
    on: boolean;
    disabled: boolean;
    ontoggle: () => void;
  }

  let { mode, status, on, disabled, ontoggle }: Props = $props();

  const running = $derived(mode === "running");
</script>

<header class="masthead" data-mode={mode}>
  <div class="masthead__head">
    <span class="masthead__brand">
      <span class="masthead__glyph"><Mark size={24} /></span>
      <span class="masthead__word">Tiles</span>
    </span>
    <span class="masthead__badge"><Chip text="alpha" size="small" /></span>
    <div class="masthead__controls">
      {#if running}<LlamaCpp />{/if}
      <Switch
        {on}
        {disabled}
        invert={running}
        label="Inference"
        onchange={ontoggle}
      />
    </div>
  </div>
  <span class="masthead__status">{status}</span>
</header>

<style>
  .masthead {
    position: relative;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 3px;
    min-height: var(--h-masthead);
    padding: 9px var(--pad-x) 10px;
    background: var(--void);
    color: var(--bone);
    transition:
      background var(--dur-state) ease-out,
      color var(--dur-state) ease-out;
  }

  /* the whole block is the status light, which is why there is no dot anywhere */
  .masthead[data-mode="running"] {
    background: var(--signal);
    color: var(--void);
  }

  .masthead[data-mode="down"] {
    color: var(--slate);
  }

  .masthead[data-mode="down"]::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: var(--rail-w);
    background: var(--alert);
  }

  .masthead__head {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  /* the site sets the mark about twice the wordmark's cap height and keeps the
     two tight, so they read as one lockup */
  .masthead__brand {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  /* the mark is lit only while inference is, a small second reading of the
     same state the ground carries */
  .masthead__glyph {
    display: flex;
    flex: none;
    transition: opacity var(--dur-state) ease-out;
  }

  .masthead:not([data-mode="running"]) .masthead__glyph {
    opacity: 0.4;
  }

  .masthead__word {
    font-size: var(--fs-display);
    font-weight: 600;
    letter-spacing: var(--tracking-display);
  }

  .masthead__badge {
    display: flex;
    flex: none;
    margin-right: auto;
  }

  /* the chip's steel would sit heavy on the yellow, this is the switch's
     inverted track instead */
  .masthead[data-mode="running"] .masthead__badge :global(.chip) {
    background: rgba(0, 0, 0, 0.12);
    color: rgba(0, 0, 0, 0.6);
  }

  .masthead__controls {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .masthead__status {
    font-size: var(--fs-body);
    color: var(--ash);
    transition: color var(--dur-state) ease-out;
  }

  .masthead[data-mode="running"] .masthead__status {
    color: rgba(0, 0, 0, 0.62);
  }

  .masthead[data-mode="down"] .masthead__status {
    color: var(--alert);
  }

  /* something is in flight and the daemon will not say how far along */
  .masthead[data-mode="starting"]::after,
  .masthead[data-mode="connecting"]::after {
    content: "";
    position: absolute;
    left: 0;
    bottom: 0;
    width: 35%;
    height: var(--hairline);
    background: linear-gradient(
      90deg,
      transparent,
      var(--signal) 42%,
      var(--signal) 58%,
      transparent
    );
    animation: sweep 1.4s linear infinite;
  }

  @keyframes sweep {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(286%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .masthead[data-mode="starting"]::after,
    .masthead[data-mode="connecting"]::after {
      width: 100%;
      background: var(--signal);
      opacity: 0.4;
      animation: none;
    }
  }
</style>
