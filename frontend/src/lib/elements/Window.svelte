<script>
  import Button from "./Button.svelte";

  let { title = "", children } = $props();
  let open = $state(false);

  function close() {
    open = false;
  }

  $effect(() => {
    if (!open) return;
    function handleKeydown(event) {
      if (event.key === "Escape") {
        event.stopPropagation();
        close();
      }
    }
    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
  });
</script>

<Button onclick={() => (open = true)} label={`Details for ${title}`}>···</Button>

{#if open}
  <button type="button" class="scrim" onclick={close} aria-label="Close"></button>
  <div class="window frame" role="dialog" aria-modal="true" aria-label={title}>
    <div class="bar-row">
      <span class="title">{title}</span>
      <button type="button" class="close" onclick={close} aria-label="Close">
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M1 1 L9 9 M9 1 L1 9" stroke="currentColor" stroke-width="1.5" fill="none" />
        </svg>
      </button>
    </div>
    <div class="body">
      {@render children()}
    </div>
  </div>
{/if}

<style>
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
    background: var(--background);
  }
  .bar-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.6rem;
    padding: 0.5rem 0.6rem;
    border-bottom: 1px solid var(--blue);
  }
  .title {
    font-size: 0.9rem;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .close {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    padding: 0;
    background: var(--light-black);
    border: 1px solid var(--blue);
    color: var(--cyan);
    cursor: pointer;
    flex-shrink: 0;
  }
  .body {
    padding: 0.9rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
</style>
