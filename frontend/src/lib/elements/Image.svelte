<script>
  let { src = "", alt = "" } = $props();
</script>

<img class="image" {src} {alt} />
<style>
  .image {
    max-width: 100%;
    display: block;
    object-fit: contain;
  }
</style>
