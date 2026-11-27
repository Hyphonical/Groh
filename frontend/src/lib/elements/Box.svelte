<script>
  let { children } = $props();
</script>

<div class="box frame">
  {@render children()}
</div>
<style>
  .box {
    background: var(--background);
    padding: 0.9rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
    width: fit-content;
    max-width: 100%;
  }
</style>
