<script lang="ts">
  interface Props {
    text: string;
    size?: "regular" | "small";
  }

  let { text, size = "regular" }: Props = $props();
</script>

<span class="chip" data-size={size}>{text}</span>

<style>
  /* the one cut echoing the logo's shear, chips only and never rows */
  .chip {
    flex: none;
    clip-path: polygon(
      0 0,
      calc(100% - var(--cut)) 0,
      100% var(--cut),
      100% 100%,
      0 100%
    );
    padding: 3px 5px;
    background: var(--steel);
    color: var(--ash);
    font-family: var(--font-mono);
    font-size: var(--fs-chip);
    font-weight: 600;
    line-height: 1;
    letter-spacing: var(--tracking-chip);
    font-variant-numeric: tabular-nums;
  }

  .chip[data-size="small"] {
    padding: 2px 4px;
    font-size: 9px;
  }
</style>
