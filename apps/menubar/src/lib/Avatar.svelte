<script lang="ts">
  let { nickname, size = 32 }: { nickname: string; size?: number } = $props();

  // no image to load, the daemon stores a did and a name and nothing else
  const initials = $derived(
    nickname
      .split(/[.\-\s_]+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((part) => part[0].toUpperCase())
      .join("") || "?",
  );
</script>

<div class="avatar" style="width: {size}px; height: {size}px; font-size: {Math.round(size * 0.375)}px">
  {initials}
</div>

<style>
  .avatar {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    border-radius: 50%;
    background: var(--fill-strong);
    color: var(--text-secondary);
    font-weight: var(--fw-semibold);
    letter-spacing: 0.02em;
  }
</style>
