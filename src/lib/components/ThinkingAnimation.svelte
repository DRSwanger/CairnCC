<script lang="ts">
  import { onMount, onDestroy } from "svelte";

  let {
    size = 220,
    class: className = "",
  }: {
    size?: number;
    class?: string;
  } = $props();

  let canvas: HTMLCanvasElement | undefined = $state();
  let animFrame: number;
  let startTime = 0;

  // Compact mode when rendered small — fewer nodes, proportional sizing
  const compact = size < 80;

  type Node = { x: number; y: number; r: number; phase: number; speed: number };
  type Edge = { a: number; b: number };
  type Pulse = { edge: number; t: number; speed: number; opacity: number };

  const NODE_COUNT = compact ? 5 : 11;
  const CONNECT_DIST = compact ? 0.72 : 0.48;
  let nodes: Node[] = [];
  let edges: Edge[] = [];
  let pulses: Pulse[] = [];

  function initGraph(w: number, h: number) {
    const cx = w / 2,
      cy = h / 2;
    nodes = [];

    if (compact) {
      // 1 centre hub + 4 outer nodes, radii proportional to canvas
      nodes.push({ x: cx, y: cy, r: w * 0.1, phase: 0, speed: 0.7 });
      for (let i = 0; i < 4; i++) {
        const a = (i / 4) * Math.PI * 2 + 0.4;
        const d = w * 0.3 + Math.random() * w * 0.04;
        nodes.push({
          x: cx + Math.cos(a) * d,
          y: cy + Math.sin(a) * d,
          r: w * 0.065 + Math.random() * w * 0.02,
          phase: Math.random() * Math.PI * 2,
          speed: 0.5 + Math.random() * 0.5,
        });
      }
    } else {
      // Original full layout
      nodes.push({ x: cx, y: cy, r: 5, phase: 0, speed: 0.7 });
      for (let i = 0; i < 5; i++) {
        const a = (i / 5) * Math.PI * 2 + 0.3;
        const d = w * 0.18 + Math.random() * w * 0.05;
        nodes.push({
          x: cx + Math.cos(a) * d,
          y: cy + Math.sin(a) * d,
          r: 3 + Math.random() * 2,
          phase: Math.random() * Math.PI * 2,
          speed: 0.5 + Math.random() * 0.5,
        });
      }
      for (let i = 0; i < 5; i++) {
        const a = (i / 5) * Math.PI * 2 + 0.9;
        const d = w * 0.32 + Math.random() * w * 0.06;
        nodes.push({
          x: cx + Math.cos(a) * d,
          y: cy + Math.sin(a) * d,
          r: 2 + Math.random() * 1.5,
          phase: Math.random() * Math.PI * 2,
          speed: 0.3 + Math.random() * 0.4,
        });
      }
    }

    edges = [];
    const dist = CONNECT_DIST * w;
    for (let i = 0; i < nodes.length; i++) {
      for (let j = i + 1; j < nodes.length; j++) {
        const dx = nodes[i].x - nodes[j].x;
        const dy = nodes[i].y - nodes[j].y;
        if (Math.sqrt(dx * dx + dy * dy) < dist) {
          edges.push({ a: i, b: j });
        }
      }
    }

    pulses = [];
    const seedCount = compact ? 3 : 6;
    for (let i = 0; i < seedCount; i++) spawnPulse();
  }

  function spawnPulse() {
    if (edges.length === 0) return;
    pulses.push({
      edge: Math.floor(Math.random() * edges.length),
      t: Math.random(),
      speed: 0.004 + Math.random() * 0.006,
      opacity: 0.6 + Math.random() * 0.4,
    });
  }

  function draw(ts: number) {
    if (!canvas) return;
    if (!startTime) startTime = ts;
    const elapsed = (ts - startTime) / 1000;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const w = canvas.width;
    const h = canvas.height;

    ctx.clearRect(0, 0, w, h);

    // Pulse glow radius — proportional in compact mode
    const pulseGlowR = compact ? w * 0.18 : 8;
    const pulseDotR = compact ? w * 0.05 : 2;
    const edgeWidth = compact ? Math.max(0.4, w * 0.012) : 0.8;
    const maxPulses = compact ? 4 : 8;

    // Edges
    for (const edge of edges) {
      const na = nodes[edge.a];
      const nb = nodes[edge.b];
      const grad = ctx.createLinearGradient(na.x, na.y, nb.x, nb.y);
      grad.addColorStop(0, "rgba(249,115,22,0.08)");
      grad.addColorStop(0.5, "rgba(249,115,22,0.18)");
      grad.addColorStop(1, "rgba(249,115,22,0.08)");
      ctx.beginPath();
      ctx.moveTo(na.x, na.y);
      ctx.lineTo(nb.x, nb.y);
      ctx.strokeStyle = grad;
      ctx.lineWidth = edgeWidth;
      ctx.stroke();
    }

    // Pulses
    const toRemove: number[] = [];
    for (let i = 0; i < pulses.length; i++) {
      const p = pulses[i];
      p.t += p.speed;
      if (p.t >= 1) {
        toRemove.push(i);
        continue;
      }
      const edge = edges[p.edge];
      const na = nodes[edge.a];
      const nb = nodes[edge.b];
      const x = na.x + (nb.x - na.x) * p.t;
      const y = na.y + (nb.y - na.y) * p.t;

      const grd = ctx.createRadialGradient(x, y, 0, x, y, pulseGlowR);
      grd.addColorStop(0, `rgba(249,115,22,${p.opacity * 0.9})`);
      grd.addColorStop(0.4, `rgba(249,115,22,${p.opacity * 0.3})`);
      grd.addColorStop(1, "rgba(249,115,22,0)");
      ctx.beginPath();
      ctx.arc(x, y, pulseGlowR, 0, Math.PI * 2);
      ctx.fillStyle = grd;
      ctx.fill();

      ctx.beginPath();
      ctx.arc(x, y, pulseDotR, 0, Math.PI * 2);
      ctx.fillStyle = `rgba(255,180,80,${p.opacity})`;
      ctx.fill();
    }
    for (let i = toRemove.length - 1; i >= 0; i--) pulses.splice(toRemove[i], 1);
    while (pulses.length < maxPulses) spawnPulse();

    // Nodes
    for (let i = 0; i < nodes.length; i++) {
      const n = nodes[i];
      const breathe = 0.7 + 0.3 * Math.sin(elapsed * n.speed + n.phase);
      // Glow radius capped to avoid blob-out at small sizes
      const glowMult = compact ? 1.5 + breathe * 0.8 : 2.5 + breathe * 1.5;
      const glowR = Math.min(n.r * glowMult, w * 0.28);
      const alpha = 0.35 + breathe * 0.25;

      const grd = ctx.createRadialGradient(n.x, n.y, 0, n.x, n.y, glowR);
      grd.addColorStop(0, `rgba(249,115,22,${alpha})`);
      grd.addColorStop(1, "rgba(249,115,22,0)");
      ctx.beginPath();
      ctx.arc(n.x, n.y, glowR, 0, Math.PI * 2);
      ctx.fillStyle = grd;
      ctx.fill();

      ctx.beginPath();
      ctx.arc(n.x, n.y, n.r * breathe, 0, Math.PI * 2);
      ctx.fillStyle =
        i === 0
          ? `rgba(255,160,60,${0.85 + breathe * 0.15})`
          : `rgba(249,115,22,${0.6 + breathe * 0.3})`;
      ctx.fill();
    }

    animFrame = requestAnimationFrame(draw);
  }

  onMount(() => {
    if (!canvas) return;
    canvas.width = size;
    canvas.height = size;
    initGraph(size, size);
    animFrame = requestAnimationFrame(draw);
  });

  onDestroy(() => {
    if (animFrame) cancelAnimationFrame(animFrame);
  });
</script>

<canvas
  bind:this={canvas}
  width={size}
  height={size}
  class="block {className}"
  style="width:{size}px;height:{size}px;"
></canvas>
