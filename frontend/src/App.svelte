<script>
  import Backdrop from "./lib/Backdrop.svelte";
  import Node from "./lib/Node.svelte";

  let columns = $state([]);
  let query = $state("");
  let searchInput = $state(null);
  let theme = $state(document.documentElement.dataset.theme === "light" ? "light" : "dark");

  function toggleTheme() {
    theme = theme === "light" ? "dark" : "light";
    document.documentElement.dataset.theme = theme;
    try {
      localStorage.setItem("groh-theme", theme);
    } catch {
      // Theme preference is best-effort; the in-memory value still applies.
    }
  }

  function handleKeydown(event) {
    if (
      event.key === "/" &&
      document.activeElement !== searchInput &&
      !(document.activeElement instanceof HTMLInputElement) &&
      !(document.activeElement instanceof HTMLTextAreaElement)
    ) {
      event.preventDefault();
      searchInput?.focus();
    }
  }

  $effect(() => {
    const source = new EventSource("/api/events");
    source.onmessage = (event) => {
      try {
        const payload = JSON.parse(event.data);
        columns = Array.isArray(payload?.columns) ? payload.columns : [];
      } catch {
        // Drop a malformed SSE frame; the next snapshot replaces it.
      }
    };
    return () => source.close();
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<Backdrop {theme} />

<main>
  <div class="toolbar">
    <input
      bind:this={searchInput}
      type="search"
      class="search frame"
      placeholder="Search..."
      aria-label="Search"
      bind:value={query}
      onkeydown={(event) => {
        if (event.key === "Escape") {
          query = "";
          searchInput?.blur();
        }
      }}
    />
    <div class="toolbar-right">
      <button type="button" class="frame theme-btn" onclick={toggleTheme} aria-label="Switch theme">
        {theme === "light" ? "DARK" : "LIGHT"}
      </button>
      <a class="gh-link frame" href="https://github.com/Hyphonical/Groh" target="_blank" rel="noopener noreferrer" title="Groh on GitHub" aria-label="Groh on GitHub">
        GH <span aria-hidden="true">&#8599;</span>
      </a>
    </div>
  </div>
  <div class="divider frame" role="separator"></div>
  <div class="layout">
    {#each columns as column, columnIndex (columnIndex)}
      <div class="page-column">
        {#each column as slot (slot.id)}
          {#if slot.tree}
            <Node node={slot.tree} {query} />
          {/if}
        {/each}
      </div>
    {/each}
  </div>
</main>

<style>
  @font-face {
    font-family: "JetBrains Mono";
    src: url("/fonts/JetBrains Mono.woff2") format("woff2");
    font-display: swap;
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
  .search {
    background: var(--background);
    color: var(--foreground);
    font-family: inherit;
    font-size: 0.8rem;
    padding: 0.25rem 0.5rem;
    width: min(260px, 100%);
  }
  .search::placeholder {
    color: var(--cyan);
  }
  .search:focus-visible {
    border-color: var(--light-cyan);
  }
  .divider {
    height: 0;
    margin-bottom: 1.75rem;
  }
  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }
  .gh-link {
    background: var(--background);
    padding: 0.25rem 0.5rem;
    font-size: 0.8rem;
    color: var(--cyan);
    text-decoration: none;
    white-space: nowrap;
  }
  .gh-link:hover,
  .gh-link:focus-visible {
    border-color: var(--light-cyan);
    color: var(--foreground);
  }
  .theme-btn {
    background: var(--background);
    padding: 0.25rem 0.5rem;
    font-size: 0.8rem;
    font-family: inherit;
    color: var(--cyan);
    cursor: pointer;
    white-space: nowrap;
  }
  .theme-btn:hover,
  .theme-btn:focus-visible {
    border-color: var(--light-cyan);
    color: var(--foreground);
  }
  .layout {
    display: flex;
    gap: 0.85rem;
    align-items: flex-start;
  }
  .page-column {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
    flex: 1;
    min-width: 0;
  }
</style>
