/**
 * Reveal animations for the drip→timeline handoff.
 *
 * CSS-only styles (none, fade, slide, blur, scale, wipe) are handled purely via
 * CSS classes in app.css. This module implements the three JS-driven animations:
 *   - fly-in:  words scatter from random directions, converge to place
 *   - decode:  characters scramble through random glyphs before settling
 *   - cascade: a bright glow wave sweeps left→right through the text
 */

import type { RevealStyle } from "$lib/stores/reveal-animation.svelte";

/** CSS-only styles — the chat page applies `reveal-{style}` class directly. */
const CSS_STYLES = new Set<RevealStyle>(["none", "fade", "slide", "blur", "scale", "wipe"]);

/** Returns true if this style needs JS orchestration. */
export function needsJsAnimation(style: RevealStyle): boolean {
  return !CSS_STYLES.has(style);
}

/** Returns the CSS class name for CSS-only styles, or empty string for JS styles. */
export function revealCssClass(style: RevealStyle): string {
  if (style === "none") return "";
  if (CSS_STYLES.has(style)) return `reveal-${style}`;
  return "";
}

// ─── Shared helpers ──────────────────────────────────────────────────

/** Collect all direct Text nodes under `root`, skipping <code>/<pre> blocks. */
function collectTextNodes(root: Element): Text[] {
  const result: Text[] = [];
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      const parent = node.parentElement;
      if (!parent) return NodeFilter.FILTER_REJECT;
      if (parent.closest("pre, code, .code-block")) return NodeFilter.FILTER_REJECT;
      if ((node.textContent ?? "").trim().length === 0) return NodeFilter.FILTER_REJECT;
      return NodeFilter.FILTER_ACCEPT;
    },
  });
  while (walker.nextNode()) result.push(walker.currentNode as Text);
  return result;
}

/** Wrap each word in a text node with a <span>, return the spans. */
function wrapWords(textNodes: Text[]): HTMLSpanElement[] {
  const spans: HTMLSpanElement[] = [];
  for (const node of textNodes) {
    const text = node.textContent ?? "";
    const parts = text.split(/(\s+)/);
    const frag = document.createDocumentFragment();
    for (const part of parts) {
      if (/^\s+$/.test(part)) {
        frag.appendChild(document.createTextNode(part));
      } else if (part.length > 0) {
        const span = document.createElement("span");
        span.textContent = part;
        span.style.display = "inline-block";
        spans.push(span);
        frag.appendChild(span);
      }
    }
    node.parentNode?.replaceChild(frag, node);
  }
  return spans;
}

/** Wrap each character in a text node with a <span>, return the spans. */
function wrapChars(textNodes: Text[]): HTMLSpanElement[] {
  const spans: HTMLSpanElement[] = [];
  for (const node of textNodes) {
    const text = node.textContent ?? "";
    const frag = document.createDocumentFragment();
    for (const ch of text) {
      if (ch === " " || ch === "\n" || ch === "\t") {
        frag.appendChild(document.createTextNode(ch));
      } else {
        const span = document.createElement("span");
        span.textContent = ch;
        span.dataset.target = ch;
        spans.push(span);
        frag.appendChild(span);
      }
    }
    node.parentNode?.replaceChild(frag, node);
  }
  return spans;
}

// ─── Random Fly-In ───────────────────────────────────────────────────

const FLY_DURATION = 500; // ms
const FLY_STAGGER = 12; // ms per word

function runFlyIn(el: Element): number {
  const textNodes = collectTextNodes(el);
  if (textNodes.length === 0) return 0;
  const spans = wrapWords(textNodes);
  if (spans.length === 0) return 0;

  // Set initial state: each word at a random offset, invisible
  for (const span of spans) {
    const angle = Math.random() * Math.PI * 2;
    const dist = 30 + Math.random() * 60;
    const tx = Math.cos(angle) * dist;
    const ty = Math.sin(angle) * dist;
    span.style.opacity = "0";
    span.style.transform = `translate(${tx}px, ${ty}px) rotate(${(Math.random() - 0.5) * 20}deg)`;
    span.style.transition = `opacity ${FLY_DURATION}ms cubic-bezier(0.22, 1, 0.36, 1), transform ${FLY_DURATION}ms cubic-bezier(0.22, 1, 0.36, 1)`;
  }

  // Stagger the reveal
  const totalTime = FLY_STAGGER * spans.length + FLY_DURATION;
  requestAnimationFrame(() => {
    spans.forEach((span, i) => {
      setTimeout(() => {
        span.style.opacity = "1";
        span.style.transform = "translate(0,0) rotate(0deg)";
      }, i * FLY_STAGGER);
    });
  });

  // Cleanup: remove inline styles after animation
  setTimeout(() => {
    for (const span of spans) {
      span.style.removeProperty("opacity");
      span.style.removeProperty("transform");
      span.style.removeProperty("transition");
      span.style.removeProperty("display");
    }
  }, totalTime + 50);

  return totalTime;
}

// ─── Decode / Scramble ───────────────────────────────────────────────

const DECODE_GLYPHS = "░▒▓█▲◆◇○●■□△▽◁▷◈◎★☆⬡⬢⚡⟐⟡⧫";
const DECODE_CYCLES = 6; // number of glyph swaps before settling
const DECODE_INTERVAL = 35; // ms between swaps
const DECODE_STAGGER = 4; // ms per character stagger

function runDecode(el: Element): number {
  const textNodes = collectTextNodes(el);
  if (textNodes.length === 0) return 0;
  const spans = wrapChars(textNodes);
  if (spans.length === 0) return 0;

  const totalTime = DECODE_STAGGER * spans.length + DECODE_CYCLES * DECODE_INTERVAL + 50;

  spans.forEach((span, i) => {
    const target = span.dataset.target ?? "";
    const delay = i * DECODE_STAGGER;
    let cycle = 0;

    span.style.color = "hsl(var(--primary))";
    span.textContent = DECODE_GLYPHS[Math.floor(Math.random() * DECODE_GLYPHS.length)];

    const iv = setInterval(() => {
      cycle++;
      if (cycle >= DECODE_CYCLES) {
        clearInterval(iv);
        span.textContent = target;
        span.style.removeProperty("color");
        return;
      }
      span.textContent = DECODE_GLYPHS[Math.floor(Math.random() * DECODE_GLYPHS.length)];
    }, DECODE_INTERVAL);

    // Delay the start per character for left-to-right sweep
    if (delay > 0) {
      clearInterval(iv);
      const timeoutId = setTimeout(() => {
        let c = 0;
        const iv2 = setInterval(() => {
          c++;
          if (c >= DECODE_CYCLES) {
            clearInterval(iv2);
            span.textContent = target;
            span.style.removeProperty("color");
            return;
          }
          span.textContent = DECODE_GLYPHS[Math.floor(Math.random() * DECODE_GLYPHS.length)];
        }, DECODE_INTERVAL);
      }, delay);
      // Store timeout for potential cleanup
      span.dataset.tid = String(timeoutId);
    }
  });

  return totalTime;
}

// ─── Signal Cascade ──────────────────────────────────────────────────

const CASCADE_DURATION = 600; // ms total sweep
const CASCADE_GLOW_WIDTH = 0.15; // fraction of total span width that glows

function runCascade(el: Element): number {
  const textNodes = collectTextNodes(el);
  if (textNodes.length === 0) return 0;
  const spans = wrapChars(textNodes);
  if (spans.length === 0) return 0;

  // Start dim, sweep a glow wave left→right
  for (const span of spans) {
    span.style.opacity = "0.3";
    span.style.transition = "none";
  }

  const total = spans.length;
  const start = performance.now();

  function frame(now: number) {
    const t = (now - start) / CASCADE_DURATION;
    if (t > 1.3) {
      // Done — clean up
      for (const span of spans) {
        span.style.opacity = "1";
        span.style.removeProperty("text-shadow");
        span.style.removeProperty("transition");
      }
      return;
    }

    for (let i = 0; i < total; i++) {
      const pos = i / total;
      const dist = Math.abs(pos - t);
      if (dist < CASCADE_GLOW_WIDTH) {
        const intensity = 1 - dist / CASCADE_GLOW_WIDTH;
        spans[i].style.opacity = String(Math.min(0.3 + intensity * 0.7, 1));
        const glow = Math.round(intensity * 8);
        spans[i].style.textShadow = `0 0 ${glow}px hsl(var(--primary) / ${intensity * 0.8})`;
      } else if (t > pos + CASCADE_GLOW_WIDTH) {
        // Wave has passed — fully visible
        spans[i].style.opacity = "1";
        spans[i].style.removeProperty("text-shadow");
      }
    }
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);

  return CASCADE_DURATION + 200;
}

// ─── Public API ──────────────────────────────────────────────────────

/**
 * Run a JS-driven reveal animation on the given element.
 * Returns the total animation duration in ms (for cleanup scheduling).
 * Only call this for styles where `needsJsAnimation()` returns true.
 */
export function runRevealAnimation(el: Element, style: RevealStyle): number {
  switch (style) {
    case "fly-in":
      return runFlyIn(el);
    case "decode":
      return runDecode(el);
    case "cascade":
      return runCascade(el);
    default:
      return 0;
  }
}
