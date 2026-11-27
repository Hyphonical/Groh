<script>
  let { tone = "other" } = $props();
</script>

<span class="dot" data-tone={tone} aria-hidden="true"></span>
<style>
  .dot {
    width: 8px;
    height: 8px;
    flex-shrink: 0;
    border: 1px solid var(--blue);
    background: transparent;
  }
  .dot[data-tone="running"] {
    background: var(--light-green);
  }
  .dot[data-tone="warn"] {
    background: var(--light-yellow);
  }
  .dot[data-tone="stopped"] {
    background: var(--light-red);
  }
</style>
