<script>
  let { tone } = $props();
</script>

<span class="status-square" data-tone={tone} aria-hidden="true"></span>

<style>
  .status-square {
    width: 8px;
    height: 8px;
    flex-shrink: 0;
    border: 1px solid var(--border);
    background: transparent;
  }

  .status-square[data-tone="running"] {
    background: var(--running);
  }

  .status-square[data-tone="warn"] {
    background: var(--warn);
  }

  .status-square[data-tone="stopped"] {
    background: var(--stopped);
  }
</style>
