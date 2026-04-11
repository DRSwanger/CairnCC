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

  // ── Node & edge config ──────────────────────────────────────────
  type Node = { x: number; y: number; r: number; phase: number; speed: number };
  type Edge = { a: number; b: number };
  type Pulse = { edge: number; t: number; speed: number; opacity: number };

  const NODE_COUNT = 11;
  const CONNECT_DIST = 0.48; // fraction of canvas size
  let nodes: Node[] = [];
  let edges: Edge[] = [];
  let pulses: Pulse[] = [];

  function initGraph(w: number, h: number) {
    // Place nodes in a rough organic cluster with one central hub
    const cx = w / 2, cy = h / 2;
    nodes = [];
    // Central hub
    nodes.push({ x: cx, y: cy, r: 5, phase: 0, speed: 0.7 });
    // Inner ring
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
    // Outer ring
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

    // Build edges: connect nodes within CONNECT_DIST
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

    // Seed initial pulses
    pulses = [];
    for (let i = 0; i < 6; i++) {
      spawnPulse();
    }
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

    // ── Draw edges ──
    for (const edge of edges) {
      const na = nodes[edge.a];
      const nb = nodes[edge.b];
      const grad = ctx.createLinearGradient(na.x, na.y, nb.x, nb.y);
      grad.addColorStop(0, "rgba(249,115,22,0.08)");
      grad.addColorStop(0.5, "rgba(249,115,22,0.15)");
      grad.addColorStop(1, "rgba(249,115,22,0.08)");
      ctx.beginPath();
      ctx.moveTo(na.x, na.y);
      ctx.lineTo(nb.x, nb.y);
      ctx.strokeStyle = grad;
      ctx.lineWidth = 0.8;
      ctx.stroke();
    }

    // ── Draw & advance pulses ──
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

      // Glow trail
      const grd = ctx.createRadialGradient(x, y, 0, x, y, 8);
      grd.addColorStop(0, `rgba(249,115,22,${p.opacity * 0.9})`);
      grd.addColorStop(0.4, `rgba(249,115,22,${p.opacity * 0.3})`);
      grd.addColorStop(1, "rgba(249,115,22,0)");
      ctx.beginPath();
      ctx.arc(x, y, 8, 0, Math.PI * 2);
      ctx.fillStyle = grd;
      ctx.fill();

      // Core dot
      ctx.beginPath();
      ctx.arc(x, y, 2, 0, Math.PI * 2);
      ctx.fillStyle = `rgba(255,180,80,${p.opacity})`;
      ctx.fill();
    }
    // Remove finished pulses (reverse to preserve indices)
    for (let i = toRemove.length - 1; i >= 0; i--) {
      pulses.splice(toRemove[i], 1);
    }
    // Spawn new pulses to maintain density
    while (pulses.length < 8) spawnPulse();

    // ── Draw nodes ──
    for (let i = 0; i < nodes.length; i++) {
      const n = nodes[i];
      const breathe = 0.7 + 0.3 * Math.sin(elapsed * n.speed + n.phase);
      const glowR = n.r * (2.5 + breathe * 1.5);
      const alpha = 0.35 + breathe * 0.25;

      // Outer glow
      const grd = ctx.createRadialGradient(n.x, n.y, 0, n.x, n.y, glowR);
      grd.addColorStop(0, `rgba(249,115,22,${alpha})`);
      grd.addColorStop(1, "rgba(249,115,22,0)");
      ctx.beginPath();
      ctx.arc(n.x, n.y, glowR, 0, Math.PI * 2);
      ctx.fillStyle = grd;
      ctx.fill();

      // Core
      ctx.beginPath();
      ctx.arc(n.x, n.y, n.r * breathe, 0, Math.PI * 2);
      ctx.fillStyle = i === 0
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
