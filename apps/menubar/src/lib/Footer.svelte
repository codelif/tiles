<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  interface Props {
    /** the daemon's version, or why there is no version to show */
    note: string;
    /** a version reads quietly, a failure does not */
    alert?: boolean;
  }

  let { note, alert = false }: Props = $props();
</script>

<footer class="footer">
  <span class="footer__note" data-alert={alert}>{note}</span>
  <button class="footer__quit" onclick={() => void invoke("quit_app").catch(() => {})}>
    Quit
  </button>
</footer>

<style>
  .footer {
    display: flex;
    align-items: center;
    gap: 8px;
    height: var(--h-nav);
    padding: 0 var(--pad-x);
    border-top: var(--hairline) solid var(--rule);
  }

  .footer__note {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--fs-mono);
    font-variant-numeric: tabular-nums;
    color: var(--slate);
  }

  .footer__note[data-alert="true"] {
    color: var(--alert);
  }

  .footer__quit {
    flex: none;
    clip-path: polygon(
      0 0,
      calc(100% - var(--cut)) 0,
      100% var(--cut),
      100% 100%,
      0 100%
    );
    padding: 4px 9px;
    border: none;
    background: var(--steel);
    color: var(--ash);
    font-family: var(--font-ui);
    font-size: var(--fs-label);
    line-height: 1;
    transition:
      background var(--dur-state) ease-out,
      color var(--dur-state) ease-out;
  }

  .footer__quit:hover {
    background: var(--signal);
    color: var(--void);
  }
</style>
