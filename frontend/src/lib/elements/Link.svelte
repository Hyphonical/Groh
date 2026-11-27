<script>
  let { href = "", children } = $props();
  let safeHref = $derived(
    /^(https?:\/\/|\/)/i.test(href) ? href : "#",
  );
  let external = $derived(/^https?:\/\//i.test(safeHref));
</script>

<a
  class="link"
  href={safeHref}
  target={external ? "_blank" : undefined}
  rel={external ? "noopener noreferrer" : undefined}
>
  {@render children()}
</a>
<style>
  .link {
    font-size: 0.8rem;
    background: var(--background);
    outline: 3px solid var(--background);
    width: fit-content;
    max-width: 100%;
    color: var(--cyan);
    text-decoration: none;
  }
  .link:hover {
    color: var(--foreground);
  }
</style>
