<script>
  import { onMount } from "svelte";
  let { theme } = $props();
  let worker = null;

  let canvas = $state(null);

  const SCALE = 4;

  function readHex(name) {
    const value = getComputedStyle(document.documentElement)
      .getPropertyValue(name)
      .trim()
      .replace("#", "");
    return [0, 2, 4].map((s) => parseInt(value.slice(s, s + 2), 16));
  }

  function size() {
    return {
      width: Math.ceil(innerWidth / SCALE),
      height: Math.ceil(innerHeight / SCALE),
    };
  }

  function colours() {
    return {
      fg: readHex("--fg"),
      bg: readHex("--bg"),
    };
  }

  onMount(() => {
    if (!canvas || !("transferControlToOffscreen" in canvas)) return;
    worker = new Worker(
      new URL("./backdrop.worker.js", import.meta.url),
      { type: "module" },
    );
    const offscreen = canvas.transferControlToOffscreen();
    const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
    worker.postMessage(
      { canvas: offscreen, still, ...size(), ...colours() },
      [offscreen],
    );
    const onResize = () => worker.postMessage(size());
    addEventListener("resize", onResize);
    return () => {
      removeEventListener("resize", onResize);
      worker.terminate();
      worker = null;
    };
  });

  $effect(() => {
    void theme;
    worker?.postMessage(colours());
  });
</script>

<canvas bind:this={canvas} class="backdrop" aria-hidden="true"></canvas>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    width: 100vw;
    height: 100vh;
    z-index: -1;
    pointer-events: none;
    image-rendering: pixelated;
  }
</style>
