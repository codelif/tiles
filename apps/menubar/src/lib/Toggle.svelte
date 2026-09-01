<script lang="ts">
  interface Props {
    on: boolean;
    disabled?: boolean;
    label: string;
    onchange: () => void;
  }

  let { on, disabled = false, label, onchange }: Props = $props();
</script>

<button
  class="toggle"
  role="switch"
  aria-checked={on}
  aria-label={label}
  data-on={on}
  {disabled}
  onclick={onchange}
>
  <span class="toggle__knob"></span>
</button>

<style>
  /* the switch is the one control that has to read as AppKit rather than web */
  .toggle {
    --toggle-w: 54px;
    --toggle-h: 24px;
    --knob-w: 32px;
    --knob-h: 20px;
    --dur-fade: 120ms;

    position: relative;
    flex: none;
    width: var(--toggle-w);
    height: var(--toggle-h);
    border: none;
    border-radius: calc(var(--toggle-h) / 2);
    background: var(--track-off);
    box-shadow: inset 0 0 0 0.5px var(--track-rim);
    transition: background var(--dur-fade) ease-out;
  }

  .toggle[data-on="true"] {
    background: var(--accent);
    box-shadow: inset 0 0 0 0.5px rgba(0, 0, 0, 0.06);
  }

  .toggle__knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: var(--knob-w);
    height: var(--knob-h);
    border-radius: calc(var(--knob-h) / 2);
    background: var(--knob-off);
    box-shadow: var(--knob-shadow);
    transition: transform var(--dur-fade) var(--ease-push);
  }

  .toggle[data-on="true"] .toggle__knob {
    transform: translateX(calc(var(--toggle-w) - var(--knob-w) - 4px));
    background: var(--knob);
  }

  @media (prefers-reduced-motion: reduce) {
    .toggle,
    .toggle__knob {
      transition: none;
    }
  }
</style>
