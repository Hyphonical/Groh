<script>
  import Backdrop from "./lib/Backdrop.svelte";
  import StatusSquare from "./lib/StatusSquare.svelte";
  import { statusTone } from "./lib/status.js";

  let containers = $state([]);
  let query = $state("");
  let selectedId = $state(null);
  let nowTick = $state(0);
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
  $effect(() => {
    const timer = setInterval(() => {
      nowTick += 1;
    }, 30000);
    return () => clearInterval(timer);
  });

  let selected = $derived.by(() => {
    if (!selectedId) return null;
    return containers.find((c) => c.id === selectedId) ?? null;
  });

  function handleKeydown(event) {
    if (event.key === "Escape") {
      if (selected) {
        closeDetails();
      } else if (document.activeElement === searchInput) {
        query = "";
        searchInput?.blur();
      }
    } else if (event.key === "/" && document.activeElement !== searchInput) {
      event.preventDefault();
      searchInput?.focus();
    }
  }

  function openDetails(container) {
    selectedId = container.id;
  }

  function closeDetails() {
    selectedId = null;
  }

  function formatBytes(bytes) {
    if (bytes == null) return "—";
    const mb = bytes / 1048576;
    if (mb < 1024) {
      if (mb < 10) return `${mb.toFixed(1)} MB`;
      return `${Math.round(mb)} MB`;
    }
    return `${(mb / 1024).toFixed(1)} GB`;
  }

  function formatCpu(percent) {
    if (percent == null) return "—";
    return `${percent.toFixed(1)}%`;
  }

  function uptimeText(startedAt) {
    void nowTick;
    if (!startedAt) return "—";
    const started = Date.parse(startedAt);
    if (Number.isNaN(started)) return "—";
    const seconds = Math.max(0, Math.floor((Date.now() - started) / 1000));
    if (seconds < 60) return `${seconds}s`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ${minutes % 60}m`;
    const days = Math.floor(hours / 24);
    return `${days}d ${hours % 24}h`;
  }

  function memPercent(item) {
    if (item?.mem_usage == null || !item?.mem_limit) return null;
    return (item.mem_usage / item.mem_limit) * 100;
  }

  function barLevel(percent) {
    if (percent == null) return "low";
    if (percent > 80) return "high";
    if (percent > 60) return "med";
    return "low";
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
    <label class="search frame">
      <span class="slash">/</span>
      <input
        bind:this={searchInput}
        bind:value={query}
        type="text"
        placeholder="filter"
        aria-label="Filter containers"
      />
    </label>
    <div class="toolbar-right">
      <div class="summary">{summary}</div>
      <a
        class="gh-link frame"
        href="https://github.com/Hyphonical/Groh"
        target="_blank"
        rel="noopener noreferrer"
        title="Groh on GitHub"
        aria-label="Groh on GitHub"
      >GH <span aria-hidden="true">&#8599;</span></a>
    </div>
  </div>
  <div class="divider frame" role="separator"></div>

  {#each sections as [category, items] (category)}
    <section class="category">
      {#if category}
        <h2 class="category-title">{category}</h2>
      {/if}
      <div class="grid">
        {#each items as c (c.id)}
          <article class="card frame" class:clickable={Boolean(c.url)}>
            <div class="card-top">
              {#if c.url}
                <a class="card-link" href={c.url}>
                  {#if c.icon}
                    <img class="icon" src={c.icon} alt="" />
                  {/if}
                  <div class="title-group">
                    <span class="name">
                      <StatusSquare tone={statusTone(c)} />
                      <span class="name-text">{c.name}</span>
                    </span>
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
                    <span class="name">
                      <StatusSquare tone={statusTone(c)} />
                      <span class="name-text">{c.name}</span>
                    </span>
                    {#if c.description}
                      <span class="desc">{c.description}</span>
                    {/if}
                  </div>
                </div>
              {/if}

              <button
                type="button"
                class="corner-btn"
                onclick={() => openDetails(c)}
                aria-label={`Details for ${c.name}`}
              >
                {c.children?.length ? `+${c.children.length}` : "···"}
              </button>
            </div>
          </article>
        {/each}
      </div>
    </section>
  {/each}
</main>

{#if selected}
  <button type="button" class="scrim" onclick={closeDetails} aria-label="Close"></button>
  <div class="window frame" role="dialog" aria-modal="true" aria-labelledby="window-title">
    <div class="window-bar">
      <span id="window-title" class="window-title">{selected.name}</span>
      <button type="button" class="close-btn" onclick={closeDetails} aria-label="Close">
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M1 1 L9 9 M9 1 L1 9" stroke="currentColor" stroke-width="1.5" fill="none" />
        </svg>
      </button>
    </div>
    <div class="window-body">
      <div class="meta-row">
        <span class="state" data-state={selected.state}>{selected.state}</span>
      </div>
      {#if selected.description}
        <p class="window-desc">{selected.description}</p>
      {/if}
      <div class="image">{selected.image}</div>
      <div class="stats">
        <div class="stat-row">
          <span class="stat-label">UPTIME {uptimeText(selected.started_at)}</span>
        </div>
        <div class="stat-row">
          <span class="stat-label">CPU {formatCpu(selected.cpu_percent)}</span>
          <span class="bar">
            <span
              class="bar-fill"
              data-level={barLevel(selected.cpu_percent)}
              style:width={`${Math.min(100, selected.cpu_percent ?? 0)}%`}
            ></span>
          </span>
        </div>
        <div class="stat-row">
          <span class="stat-label">
            MEM {formatBytes(selected.mem_usage)} / {formatBytes(selected.mem_limit)}
          </span>
          <span class="bar">
            <span
              class="bar-fill"
              data-level={barLevel(memPercent(selected))}
              style:width={`${Math.min(100, memPercent(selected) ?? 0)}%`}
            ></span>
          </span>
        </div>
      </div>
      {#if selected.children?.length}
        <div class="children">
          {#each selected.children as child (child.id)}
            <div class="child-row">
              <span class="child-id">
                <StatusSquare tone={statusTone(child)} />
                <span class="child-name">{child.name}</span>
              </span>
              <span class="state" data-state={child.state}>{child.state}</span>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
{/if}

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
    --running: #ccd88c;
    --stopped: #de7c4c;
    --warn: #e2c47e;
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

  .frame {
    border: 1px solid var(--border);
    outline: 3px solid var(--bg);
    outline-offset: 0;
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
    padding-bottom: 0.75rem;
  }

  .divider {
    height: 0;
    margin-bottom: 1.75rem;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: var(--bg);
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
    background: var(--bg);
    padding: 0.25rem 0.5rem;
    font-size: 0.8rem;
    color: var(--dim);
    white-space: nowrap;
  }

  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .gh-link {
    background: var(--bg);
    padding: 0.25rem 0.5rem;
    font-size: 0.8rem;
    color: var(--dim);
    text-decoration: none;
    white-space: nowrap;
  }

  .gh-link:hover,
  .gh-link:focus-visible {
    border-color: var(--border-hover);
    color: var(--fg);
  }

  .category {
    margin-bottom: 2rem;
  }

  .category-title {
    display: inline-block;
    background: var(--bg);
    padding: 0.2rem 0.5rem 0.2rem 0;
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
    display: flex;
    align-items: center;
    gap: 0.45rem;
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--fg);
    min-width: 0;
  }

  .name-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
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

  .image {
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
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    font-size: 0.76rem;
  }

  .child-id {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    min-width: 0;
  }

  .child-name {
    color: var(--fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .scrim {
    position: fixed;
    inset: 0;
    z-index: 9;
    padding: 0;
    border: 0;
    background: rgba(0, 0, 0, 0.6);
    cursor: pointer;
  }

  .window {
    position: fixed;
    z-index: 10;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(480px, calc(100vw - 3rem));
    background: var(--bg);
  }

  .window-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.6rem;
    padding: 0.5rem 0.6rem;
    border-bottom: 1px solid var(--border);
  }

  .window-title {
    font-size: 0.9rem;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .close-btn {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    padding: 0;
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--dim);
    cursor: pointer;
    flex-shrink: 0;
  }

  .close-btn:hover {
    border-color: var(--border-hover);
    color: var(--fg);
  }

  .window-body {
    padding: 0.9rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .window-desc {
    margin: 0;
    font-size: 0.85rem;
    color: var(--fg);
  }

  .stats {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding-top: 0.45rem;
    border-top: 1px dashed var(--border);
  }

  .stat-row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    font-size: 0.78rem;
  }

  .stat-label {
    color: var(--dim);
    white-space: nowrap;
  }

  .bar {
    flex: 1;
    height: 6px;
    border: 1px solid var(--border);
    background: var(--surface);
    min-width: 0;
  }

  .bar-fill {
    display: block;
    height: 100%;
    background: var(--running);
  }

  .bar-fill[data-level="med"] {
    background: var(--warn);
  }

  .bar-fill[data-level="high"] {
    background: var(--stopped);
  }

  @media (max-width: 560px) {
    main {
      padding: 1rem 1rem 2rem;
    }

    .toolbar {
      flex-wrap: wrap;
    }

    .search {
      flex: 1 1 auto;
      min-width: 0;
    }

    .grid {
      grid-template-columns: 1fr;
    }

    .close-btn {
      position: relative;
    }

    .close-btn::after {
      content: "";
      position: absolute;
      inset: -8px;
    }
  }

  @media (max-width: 420px) {
    .search {
      display: none;
    }
  }
</style>
