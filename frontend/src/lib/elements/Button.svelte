<script>
  let { children, onclick = null, label = "" } = $props();
</script>

<button type="button" class="button" {onclick} aria-label={label || undefined}>
  {@render children()}
</button>
<style>
  .button {
    background: var(--light-black);
    border: 1px solid var(--blue);
    color: var(--cyan);
    font-family: inherit;
    font-size: 0.72rem;
    padding: 0.15rem 0.4rem;
    cursor: pointer;
    flex-shrink: 0;
  }
  .button:hover {
    border-color: var(--light-cyan);
    color: var(--foreground);
  }
</style>
