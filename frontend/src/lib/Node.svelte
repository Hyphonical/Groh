<script>
  import Text from "./elements/Text.svelte";
  import Box from "./elements/Box.svelte";
  import Row from "./elements/Row.svelte";
  import Column from "./elements/Column.svelte";
  import Separator from "./elements/Separator.svelte";
  import Bar from "./elements/Bar.svelte";
  import StatusDot from "./elements/StatusDot.svelte";
  import Link from "./elements/Link.svelte";
  import Image from "./elements/Image.svelte";
  import Icon from "./elements/Icon.svelte";
  import Window from "./elements/Window.svelte";
  import Node from "./Node.svelte";

  let { node, query = "" } = $props();

  function hasTextMatch(item, needle) {
    switch (item.type) {
      case "Text":
        return item.value.toLowerCase().includes(needle);
      case "Link":
        return item.label.toLowerCase().includes(needle);
      case "Image":
      case "Icon":
        return item.alt.toLowerCase().includes(needle);
      case "Window":
        return (
          item.title.toLowerCase().includes(needle) ||
          item.children.some((child) => hasTextMatch(child, needle))
        );
      case "Box":
      case "Row":
      case "Column":
        return item.children.some((child) => hasTextMatch(child, needle));
      default:
        return false;
    }
  }

  let needle = $derived(query.trim().toLowerCase());
  let visible = $derived(!needle || hasTextMatch(node, needle));
</script>

{#if visible}
  {#if node.type === "Text"}
    <Text value={node.value} size={node.size} color={node.color} bold={node.bold} />
  {:else if node.type === "Box"}
    <Box>
      {#each node.children as child, index (index)}
        <Node node={child} {query} />
      {/each}
    </Box>
  {:else if node.type === "Row"}
    <Row gap={node.gap}>
      {#each node.children as child, index (index)}
        <Node node={child} {query} />
      {/each}
    </Row>
  {:else if node.type === "Column"}
    <Column gap={node.gap}>
      {#each node.children as child, index (index)}
        <Node node={child} {query} />
      {/each}
    </Column>
  {:else if node.type === "Separator"}
    <Separator kind={node.kind} orientation={node.orientation} />
  {:else if node.type === "Bar"}
    <Bar percent={node.percent} color={node.color} />
  {:else if node.type === "StatusDot"}
    <StatusDot tone={node.tone} />
  {:else if node.type === "Link"}
    <Link href={node.href}>{node.label}</Link>
  {:else if node.type === "Image"}
    <Image src={node.src} alt={node.alt} />
  {:else if node.type === "Icon"}
    <Icon src={node.src} alt={node.alt} />
  {:else if node.type === "Window"}
    <Window title={node.title}>
      {#each node.children as child, index (index)}
        <Node node={child} {query} />
      {/each}
    </Window>
  {/if}
{/if}
