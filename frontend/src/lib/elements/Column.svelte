<script>
  let { children, gap = 8 } = $props();
</script>

<div class="column" style:gap={`${gap}px`}>
  {@render children()}
</div>
<style>
  .column {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
</style>
