const RANK = { other: 0, running: 1, warn: 2, stopped: 3 };

export function statusTone(container) {
  let tone = "other";
  const visit = (c) => {
    const stopped = ["exited", "stopped", "dead"].includes(c.state);
    const warn =
      c.detail?.toLowerCase().includes("unhealthy") ||
      c.state === "paused" ||
      c.state === "restarting";
    const next = stopped ? "stopped" : warn ? "warn" : c.state === "running" ? "running" : "other";
    if (RANK[next] > RANK[tone]) tone = next;
    for (const child of c.children ?? []) visit(child);
  };
  visit(container);
  return tone;
}
