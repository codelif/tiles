<script lang="ts">
  let { text }: { text: string } = $props();
</script>

<div class="section-label">{text}</div>

<style>
  .section-label {
    padding: 0 var(--content-pad-x) 4px;
    font-size: var(--fs-small);
    line-height: 13px;
    color: var(--text-tertiary);
  }
</style>
