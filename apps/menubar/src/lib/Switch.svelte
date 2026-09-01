<script lang="ts">
  interface Props {
    on: boolean;
    disabled?: boolean;
    /** a request is out and the daemon has not answered yet */
    pending?: boolean;
    /** the ground behind it is yellow, so the plate has to be the dark half */
    invert?: boolean;
    label: string;
    onchange: () => void;
  }

  let {
    on,
    disabled = false,
    pending = false,
    invert = false,
    label,
    onchange,
  }: Props = $props();
</script>

<!-- three plain rects skewed as one, rather than three clip-paths: the slant
     then cannot drift between the plate and the block riding inside it -->
<button
  class="switch"
  role="switch"
  aria-checked={on}
  aria-busy={pending}
  aria-label={label}
  data-on={on}
  data-pending={pending}
  data-invert={invert}
  {disabled}
  onclick={onchange}
>
  <span class="switch__ring">
    <!-- the masthead sweeps a light along its edge, the ring runs the same
         light around itself. one pattern for "asked, not answered" -->
    {#if pending}<span class="switch__spin"></span>{/if}
    <span class="switch__fill">
      <span class="switch__knob"></span>
    </span>
  </span>
</button>

<style>
  .switch {
    /* the shear is the logo's, 8 over 22 */
    --slant: -20deg;
    --plate-w: 38px;
    --plate-h: 22px;
    --overhang: 4px;
    --ring: 1px;
    --gap: 2px;
    --knob-w: 15px;
    --travel: 17px;
    /* a filter would composite into its own buffer and leave a visible
       rectangle on the panel's black, so the glow is the plate's own shadow */
    --glow: 0 0 0 rgba(247, 255, 97, 0);

    position: relative;
    flex: none;
    width: calc(var(--plate-w) + var(--overhang) * 2);
    height: var(--plate-h);
    border: none;
    background: none;
  }

  .switch__ring {
    position: absolute;
    inset: 0 var(--overhang);
    overflow: hidden;
    background: rgba(255, 255, 255, 0.16);
    /* overflow clips the children, never the element's own shadow */
    box-shadow: var(--glow);
    transform: skewX(var(--slant));
    transition:
      background var(--dur-state) ease-out,
      box-shadow var(--dur-state) ease-out;
  }

  .switch__fill {
    position: absolute;
    inset: var(--ring);
    background: var(--steel);
    transition: background var(--dur-state) ease-out;
  }

  /* inset on all four sides, so the plate reads as a frame the block sits in */
  .switch__knob {
    position: absolute;
    top: var(--gap);
    bottom: var(--gap);
    left: var(--gap);
    width: var(--knob-w);
    background: var(--slate);
    transition:
      transform var(--dur-state) var(--ease-push),
      background var(--dur-state) ease-out;
  }

  .switch[data-on="true"] .switch__ring,
  .switch[data-on="true"] .switch__fill {
    background: var(--signal);
  }

  .switch[data-on="true"] .switch__knob {
    transform: translateX(var(--travel));
    background: var(--void);
  }

  .switch[data-on="true"] {
    --glow:
      0 0 10px rgba(247, 255, 97, 0.65),
      0 0 20px rgba(247, 255, 97, 0.3);
  }

  /* it has to read as a button before it is pressed, not only after */
  .switch:hover:not(:disabled) {
    --glow: 0 0 8px rgba(247, 255, 97, 0.4);
  }

  .switch[data-on="true"]:hover:not(:disabled) {
    --glow:
      0 0 13px rgba(247, 255, 97, 0.8),
      0 0 24px rgba(247, 255, 97, 0.4);
  }

  .switch[data-invert="true"] .switch__ring {
    background: rgba(0, 0, 0, 0.35);
  }

  .switch[data-invert="true"] .switch__fill {
    background: rgba(0, 0, 0, 0.14);
  }

  .switch[data-invert="true"] .switch__knob {
    background: rgba(0, 0, 0, 0.35);
  }

  .switch[data-invert="true"][data-on="true"] .switch__ring,
  .switch[data-invert="true"][data-on="true"] .switch__fill {
    background: var(--void);
  }

  .switch[data-invert="true"][data-on="true"] .switch__knob {
    background: var(--signal);
  }

  /* yellow on yellow is nothing, so on that ground the plate throws a dark
     halo instead. same affordance, the only colour the ground leaves */
  .switch[data-invert="true"] {
    --glow: 0 0 10px rgba(0, 0, 0, 0.35);
  }

  .switch[data-invert="true"]:hover:not(:disabled) {
    --glow: 0 0 14px rgba(0, 0, 0, 0.5);
  }

  .switch:disabled {
    opacity: 0.35;
  }

  /* the fill does not claim the new state until the daemon confirms it, which
     is also what makes the light legible */
  .switch[data-pending="true"] {
    --glow: 0 0 0 rgba(247, 255, 97, 0);
  }

  .switch[data-pending="true"] .switch__ring,
  .switch[data-pending="true"] .switch__fill {
    background: var(--steel);
  }

  .switch[data-pending="true"] .switch__knob {
    transform: none;
    background: var(--slate);
  }

  /* the ring is the hairline the fill leaves uncovered, so a cone spinning
     under the fill lights that hairline and nothing else */
  .switch__spin {
    position: absolute;
    top: 50%;
    left: 50%;
    width: 200%;
    aspect-ratio: 1;
    translate: -50% -50%;
    background: conic-gradient(
      from 0turn,
      transparent 0 0.55turn,
      var(--signal) 0.82turn,
      transparent 0.95turn 1turn
    );
    animation: spin 1.2s linear infinite;
  }

  @keyframes spin {
    to {
      rotate: 1turn;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .switch__ring,
    .switch__fill,
    .switch__knob {
      transition: none;
    }

    /* a still ring says the same thing without the travel */
    .switch__spin {
      animation: none;
      background: var(--signal);
      opacity: 0.45;
    }
  }
</style>
