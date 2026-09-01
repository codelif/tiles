<script lang="ts">
  import glyph from "../assets/marks/llamacpp.svg?raw";
</script>

<!-- the engine behind inference, so it shows only while inference is up -->
<span class="llama" title="Inference by llama.cpp">{@html glyph}</span>

<style>
  .llama {
    flex: none;
    display: block;
    width: 11px;
    height: 11px;
    opacity: 0.55;
  }

  .llama :global(svg) {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
