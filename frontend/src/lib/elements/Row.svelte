<script>
  let { children, gap = 8 } = $props();
</script>

<div class="row" style:gap={`${gap}px`}>
  {@render children()}
</div>
<style>
  .row {
    display: flex;
    align-items: center;
    min-width: 0;
  }
</style>
