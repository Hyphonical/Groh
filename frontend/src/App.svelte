<script>
  import Backdrop from "./lib/Backdrop.svelte";

  let containers = $state([]);
  let query = $state("");
  let expanded = $state({});
  let searchInput = $state(null);

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

  function handleKeydown(event) {
    if (event.key === "/" && document.activeElement !== searchInput) {
      event.preventDefault();
      searchInput?.focus();
    } else if (event.key === "Escape" && document.activeElement === searchInput) {
      query = "";
      searchInput?.blur();
    }
  }

  function toggleExpand(id, event) {
    event.preventDefault();
    event.stopPropagation();
    expanded[id] = !expanded[id];
  }

  let filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return containers;
    return containers.filter((c) => {
      const hay = [c.name, c.description ?? "", c.category ?? ""]
        .join(" ")
        .toLowerCase();
      return hay.includes(q);
    });
  });

  let sections = $derived.by(() => {
    const map = new Map();
    for (const c of filtered) {
      const key = c.category?.trim() || "";
      if (!map.has(key)) map.set(key, []);
      map.get(key).push(c);
    }
    return Array.from(map.entries()).sort(([a], [b]) => {
      if (a === "" && b !== "") return 1;
      if (a !== "" && b === "") return -1;
      return a.localeCompare(b);
    });
  });

  let summary = $derived.by(() => {
    let running = 0;
    let exited = 0;
    let unhealthy = 0;
    const count = (c) => {
      if (c.detail?.toLowerCase().includes("unhealthy")) {
        unhealthy += 1;
      } else if (c.state === "running") {
        running += 1;
      } else if (c.state === "exited" || c.state === "stopped" || c.state === "dead") {
        exited += 1;
      }
      for (const child of c.children ?? []) count(child);
    };
    for (const c of containers) count(c);
    const parts = [`${running} running`];
    if (exited > 0) parts.push(`${exited} exited`);
    if (unhealthy > 0) parts.push(`${unhealthy} unhealthy`);
    return parts.join(" · ");
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<Backdrop />

<main>
  <div class="toolbar">
    <label class="search">
      <span class="slash">/</span>
      <input
        bind:this={searchInput}
        bind:value={query}
        type="text"
        placeholder="filter"
        aria-label="Filter containers"
      />
    </label>
    <div class="summary">{summary}</div>
  </div>

  {#each sections as [category, items] (category)}
    <section class="category">
      {#if category}
        <h2 class="category-title">{category}</h2>
      {/if}
      <div class="grid">
        {#each items as c (c.id)}
          <article class="card" class:clickable={Boolean(c.url)}>
            <div class="card-top">
              {#if c.url}
                <a class="card-link" href={c.url}>
                  {#if c.icon}
                    <img class="icon" src={c.icon} alt="" />
                  {/if}
                  <div class="title-group">
                    <span class="name">{c.name}</span>
                    {#if c.description}
                      <span class="desc">{c.description}</span>
                    {/if}
                  </div>
                </a>
              {:else}
                <div class="card-link">
                  {#if c.icon}
                    <img class="icon" src={c.icon} alt="" />
                  {/if}
                  <div class="title-group">
                    <span class="name">{c.name}</span>
                    {#if c.description}
                      <span class="desc">{c.description}</span>
                    {/if}
                  </div>
                </div>
              {/if}

              <button
                type="button"
                class="corner-btn"
                aria-expanded={Boolean(expanded[c.id])}
                onclick={(e) => toggleExpand(c.id, e)}
              >
                {c.children?.length ? `+${c.children.length}` : "···"}
              </button>
            </div>

            {#if expanded[c.id]}
              <div class="details">
                <div class="meta-row">
                  <span class="state" data-state={c.state}>{c.state}</span>
                  <span class="detail">{c.detail}</span>
                </div>
                <div class="image">{c.image}</div>
                {#if c.children?.length}
                  <div class="children">
                    {#each c.children as child (child.id)}
                      <div class="child-row">
                        <span class="child-name">{child.name}</span>
                        <span class="state" data-state={child.state}>{child.state}</span>
                      </div>
                    {/each}
                  </div>
                {/if}
              </div>
            {/if}
          </article>
        {/each}
      </div>
    </section>
  {/each}
</main>

<style>
  @font-face {
    font-family: "JetBrains Mono";
    src: url("/fonts/JetBrains Mono.woff2") format("woff2");
    font-display: swap;
  }

  :global(:root) {
    --bg: #141414;
    --surface: #262626;
    --fg: #ffffd4;
    --dim: #778385;
    --border: #44474a;
    --border-hover: #8a989b;
    --running: #afb97a;
    --stopped: #c06d44;
    --warn: #c2a86c;
    caret-color: #ffffff;
  }

  :global(::selection) {
    background: #313131;
    color: #ffffd4;
  }

  :global(html) {
    background-color: var(--bg);
    color: var(--fg);
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
    padding: 1.5rem 1.5rem 3rem;
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    margin-bottom: 1.75rem;
    padding-bottom: 0.75rem;
    border-bottom: 1px solid var(--border);
  }

  .search {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: var(--bg);
    border: 1px solid var(--border);
    padding: 0.35rem 0.6rem;
    min-width: 220px;
  }

  .search:focus-within {
    border-color: var(--border-hover);
  }

  .slash {
    color: var(--dim);
    font-size: 0.85rem;
  }

  .search input {
    background: transparent;
    border: none;
    outline: none;
    color: var(--fg);
    font-family: inherit;
    font-size: 0.85rem;
    width: 100%;
  }

  .search input::placeholder {
    color: var(--dim);
  }

  .summary {
    font-size: 0.8rem;
    color: var(--dim);
    white-space: nowrap;
  }

  .category {
    margin-bottom: 2rem;
  }

  .category-title {
    margin: 0 0 0.75rem;
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--dim);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 0.85rem;
  }

  .card {
    background: var(--bg);
    border: 1px solid var(--border);
    padding: 0.9rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
  }

  .card:hover {
    border-color: var(--border-hover);
    background: var(--surface);
  }

  .card-top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.6rem;
  }

  .card-link {
    display: flex;
    align-items: flex-start;
    gap: 0.65rem;
    text-decoration: none;
    color: inherit;
    min-width: 0;
    flex: 1;
  }

  .icon {
    width: 20px;
    height: 20px;
    object-fit: contain;
    flex-shrink: 0;
    margin-top: 0.1rem;
  }

  .title-group {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    min-width: 0;
  }

  .name {
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .desc {
    font-size: 0.78rem;
    color: var(--dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .corner-btn {
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--dim);
    font-family: inherit;
    font-size: 0.72rem;
    padding: 0.15rem 0.4rem;
    cursor: pointer;
    flex-shrink: 0;
  }

  .corner-btn:hover {
    border-color: var(--border-hover);
    color: var(--fg);
  }

  .details {
    border-top: 1px solid var(--border);
    padding-top: 0.6rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .meta-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .state {
    font-size: 0.72rem;
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
    font-size: 0.75rem;
    color: var(--dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .children {
    margin-top: 0.35rem;
    padding-top: 0.45rem;
    border-top: 1px dashed var(--border);
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .child-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.5rem;
    font-size: 0.76rem;
  }

  .child-name {
    color: var(--fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
