<script>
  let { value = "", size = "normal", color = "foreground", bold = false } = $props();
</script>

<span class="text" data-size={size} data-color={color} class:bold>{value}</span>
<style>
  .text {
    font-size: 0.85rem;
    background: var(--background);
    outline: 3px solid var(--background);
    width: fit-content;
    max-width: 100%;
    overflow-wrap: anywhere;
  }
  .text[data-size="small"] {
    font-size: 0.75rem;
  }
  .text[data-size="large"] {
    font-size: 1.4rem;
  }
  .bold {
    font-weight: 600;
  }
</style>
