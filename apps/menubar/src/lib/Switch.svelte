<script lang="ts">
  interface Props {
    on: boolean;
    disabled?: boolean;
    /** a request is out and the daemon has not answered yet */
    pending?: boolean;
    /** the ground behind it is yellow, so the track has to be the dark half */
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
  <span class="switch__knob"></span>
  {#if pending}
    <!-- the masthead sweeps a light along its edge, a switch runs the same
         light around its border. one pattern for "asked, not answered" -->
    <span class="switch__halo"></span>
  {/if}
</button>

<style>
  .switch {
    --track-w: 40px;
    --track-h: 20px;
    --knob: 16px;

    position: relative;
    flex: none;
    width: var(--track-w);
    height: var(--track-h);
    border: none;
    border-radius: 2px;
    background: var(--steel);
    box-shadow: inset 0 0 0 1px var(--rule);
    transition:
      background var(--dur-state) ease-out,
      box-shadow var(--dur-state) ease-out;
  }

  .switch__knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: var(--knob);
    height: var(--knob);
    border-radius: 1px;
    background: var(--slate);
    transition:
      transform var(--dur-state) var(--ease-push),
      background var(--dur-state) ease-out;
  }

  .switch[data-on="true"] {
    background: var(--signal);
    box-shadow: none;
  }

  .switch[data-on="true"] .switch__knob {
    transform: translateX(calc(var(--track-w) - var(--knob) - 4px));
    background: var(--void);
  }

  .switch[data-invert="true"] {
    background: rgba(0, 0, 0, 0.14);
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.22);
  }

  .switch[data-invert="true"][data-on="true"] {
    background: var(--void);
    box-shadow: none;
  }

  .switch[data-invert="true"][data-on="true"] .switch__knob {
    background: var(--signal);
  }

  .switch:disabled {
    opacity: 0.35;
  }

  /* the fill does not claim the new state until the daemon confirms it, which
     is also what makes the light legible */
  .switch[data-pending="true"] {
    background: var(--steel);
    box-shadow: inset 0 0 0 1px var(--rule);
  }

  .switch[data-pending="true"] .switch__knob {
    background: var(--slate);
  }

  /* a ring cut out of a spinning cone, so the light hugs the corners too */
  .switch__halo {
    position: absolute;
    inset: -1px;
    padding: 1.5px;
    border-radius: 3px;
    overflow: hidden;
    pointer-events: none;
    -webkit-mask:
      linear-gradient(#000 0 0) content-box,
      linear-gradient(#000 0 0);
    mask:
      linear-gradient(#000 0 0) content-box,
      linear-gradient(#000 0 0);
    -webkit-mask-composite: xor;
    mask-composite: exclude;
  }

  .switch__halo::before {
    content: "";
    position: absolute;
    top: 50%;
    left: 50%;
    width: 150%;
    aspect-ratio: 1;
    translate: -50% -50%;
    background: conic-gradient(
      from 0turn,
      transparent 0 0.55turn,
      var(--signal) 0.82turn,
      transparent 0.95turn 1turn
    );
    filter: blur(0.5px);
    animation: halo 1.2s linear infinite;
  }

  @keyframes halo {
    to {
      rotate: 1turn;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .switch,
    .switch__knob {
      transition: none;
    }

    /* a still ring says the same thing without the travel */
    .switch__halo::before {
      animation: none;
      background: var(--signal);
      opacity: 0.45;
    }
  }
</style>
