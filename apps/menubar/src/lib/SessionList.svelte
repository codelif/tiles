<script lang="ts">
  import Row from "./Row.svelte";
  import { relativeTime } from "./time";
  import type { Session } from "../state.svelte";

  let { sessions }: { sessions: Session[] } = $props();
</script>

{#each sessions as session (session.id)}
  <!-- no onselect, there is no route that opens one -->
  <Row size="large" title={session.name} sub={relativeTime(session.createdAt)} />
{/each}
