<script>
  let { kind = "dotted", orientation = "horizontal" } = $props();
</script>

{#if orientation === "vertical"}
  <div
    class="separator-vertical"
    data-kind={kind}
    role="separator"
    aria-orientation="vertical"
  ></div>
{:else}
  <hr class="separator" data-kind={kind} />
{/if}

<style>
  .separator {
    border: 0;
    border-top: 1px solid var(--blue);
    margin: 0;
  }
  .separator[data-kind="dotted"] {
    border-top-style: dotted;
  }
  .separator[data-kind="dashed"] {
    border-top-style: dashed;
  }
  .separator-vertical {
    align-self: stretch;
    border: 0;
    border-left: 1px solid var(--blue);
    margin: 0;
  }
  .separator-vertical[data-kind="dotted"] {
    border-left-style: dotted;
  }
  .separator-vertical[data-kind="dashed"] {
    border-left-style: dashed;
  }
</style>
