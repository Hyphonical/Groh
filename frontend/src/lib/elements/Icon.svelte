<script>
  let { src = "", alt = "" } = $props();
</script>

<img class="icon" {src} {alt} />
<style>
  .icon {
    width: 20px;
    height: 20px;
    object-fit: contain;
    flex-shrink: 0;
  }
</style>
