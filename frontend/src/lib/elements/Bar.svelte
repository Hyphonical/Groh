<script>
  let { percent = 0, color = "light-green" } = $props();
  let width = $derived(Math.min(100, Math.max(0, Number(percent ?? 0))));
</script>

<span class="bar">
  <span class="fill" data-color={color} style:width={`${width}%`}></span>
</span>
<style>
  .bar {
    display: block;
    height: 6px;
    color: var(--light-green);
    background: var(--light-black);
    border: 1px solid var(--blue);
    overflow: hidden;
    flex: 1;
    min-width: 80px;
  }
  .fill {
    display: block;
    height: 100%;
    background: currentColor;
  }
</style>
