<script>
  import Backdrop from "./lib/Backdrop.svelte";

  let containers = $state([]);

  $effect(() => {
    const source = new EventSource("/api/events");
    source.onmessage = (event) => {
      try {
        containers = JSON.parse(event.data);
      } catch {
      }
    };
    return () => source.close();
  });
</script>

<Backdrop />

<main>
  <section class="grid">
    {#each containers as c (c.id)}
      <article class="card">
        <div class="row">
          <span class="name">{c.name}</span>
          <span class="state" data-state={c.state}>{c.state}</span>
        </div>
        <div class="image">{c.image}</div>
        <div class="detail">{c.detail}</div>
      </article>
    {/each}
  </section>
</main>

<style>
  @font-face {
    font-family: "JetBrains Mono";
    src: url("/fonts/JetBrains Mono.woff2") format("woff2");
    font-display: swap;
  }

  :global(:root) {
    --paper: #141414;
    --ink: #ffffd4;
    --dim: #778385;
    --running: #afb97a;
    --stopped: #c06d44;
    caret-color: #ffffff;
  }

  :global(::selection) {
    background: #313131;
    color: #ffffd4;
  }

  :global(html) {
    background-color: var(--paper);
    color: var(--ink);
    font-family: "JetBrains Mono", monospace;
  }

  :global(body) {
    margin: 0;
    min-height: 100vh;
  }

  main {
    position: relative;
    z-index: 1;
    max-width: 1120px;
    margin: 0 auto;
    padding: 2rem 1.5rem;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 1rem;
  }

  .card {
    background: color-mix(in srgb, var(--paper) 88%, var(--ink) 4%);
    border: 1px solid color-mix(in srgb, var(--dim) 28%, transparent);
    padding: 1rem 1.1rem;
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }

  .row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.75rem;
  }

  .name {
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .state {
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--dim);
    flex-shrink: 0;
  }

  .state[data-state="running"] {
    color: var(--running);
  }

  .state[data-state="exited"],
  .state[data-state="stopped"],
  .state[data-state="dead"] {
    color: var(--stopped);
  }

  .image,
  .detail {
    font-size: 0.78rem;
    color: var(--dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
