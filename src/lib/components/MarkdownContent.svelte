<script lang="ts">
  import { onMount } from "svelte";
  import { renderMarkdown, renderMermaidBlocks } from "$lib/utils/markdown";
  import { readFileBase64 } from "$lib/api";
  import { dbg, dbgWarn } from "$lib/utils/debug";
  import type { RevealStyle } from "$lib/stores/reveal-animation.svelte";

  let {
    text = "",
    streaming = false,
    basePath = "",
    class: className = "",
    draining = $bindable(false),
    rate = 35,
    revealStyle = "slide" as RevealStyle,
  }: {
    text?: string;
    streaming?: boolean;
    basePath?: string;
    class?: string;
    draining?: boolean;
    rate?: number;
    revealStyle?: RevealStyle;
  } = $props();

  let container: HTMLDivElement | undefined = $state();
  let scrambleEl: HTMLSpanElement | undefined = $state();

  // ── Style categories ──
  // "full-text" styles render all text immediately; reveal is controlled by JS/CSS, not dripText.
  const FULL_TEXT_STYLES = new Set<RevealStyle>(["fly-in", "cascade"]);
  let useFullText = $derived(FULL_TEXT_STYLES.has(revealStyle));

  // ── Drip state ──
  const MAX_ELAPSED = 50;
  let dripText = $state(streaming ? "" : text);

  // Reset when component is reused for a new message
  $effect.pre(() => {
    if (dripText && !text.startsWith(dripText)) {
      dripText = streaming ? "" : text;
    }
    if (!useFullText && text.length > dripText.length) draining = true;
  });

  // ── Decode glyphs ──
  const DECODE_GLYPHS = "░▒▓█▲◆◇○●■□△▽◁▷◈◎★☆⬡⬢⚡⟐⟡⧫";
  const DECODE_TAIL_LEN = 24;
  // Pre-allocated buffer — avoid per-frame string concat allocations.
  const _glyphChars = new Array<string>(DECODE_TAIL_LEN);
  function writeScramble(n: number) {
    if (!scrambleEl) return;
    if (n <= 0) {
      // eslint-disable-next-line svelte/no-dom-manipulating
      if (scrambleEl.firstChild) scrambleEl.textContent = "";
      return;
    }
    const cap = n < DECODE_TAIL_LEN ? n : DECODE_TAIL_LEN;
    for (let i = 0; i < cap; i++) {
      _glyphChars[i] = DECODE_GLYPHS[(Math.random() * DECODE_GLYPHS.length) | 0];
    }
    // join the slice once — single allocation per frame instead of N concatenations.
    // Direct text mutation (vs reactive {@html}) is deliberate: it avoids
    // re-deriving `html`, which would re-trigger every enrichment effect.
    // eslint-disable-next-line svelte/no-dom-manipulating
    scrambleEl.textContent = _glyphChars.slice(0, cap).join("");
  }

  // ── Full-text reveal position (0..1) for cascade/fly-in ──
  let revealFrac = $state(streaming ? 0 : 1);

  // Reset revealFrac when text resets (new message)
  $effect.pre(() => {
    if (useFullText && streaming && revealFrac > 0 && text.length < 10) {
      revealFrac = 0;
    }
  });

  // ── rAF lifecycle ──
  // Run the loop only while there's work to do; cancel when caught up. A
  // chat with N completed assistant messages otherwise keeps N rAF callbacks
  // ticking every frame forever, which the browser scheduler is happy to
  // ignore but the GC and tab-throttling heuristics are not.
  let rafId: number | undefined;
  let rafActive = false;
  let unmounted = false;

  function workRemaining(): boolean {
    if (useFullText) return text.length > 0 && revealFrac < 1;
    return dripText.length < text.length;
  }

  function startRaf() {
    if (rafActive || unmounted) return;
    if (!workRemaining()) return;
    rafActive = true;
    let lastTime = performance.now();
    let remainder = 0;

    const loop = (now: number) => {
      const elapsed = Math.min(now - lastTime, MAX_ELAPSED);
      const streamRate = streaming ? rate : rate * 2;
      const ideal = remainder + (elapsed / 1000) * streamRate;
      const chars = Math.floor(ideal);
      remainder = ideal - chars;
      lastTime = now;

      if (useFullText) {
        // Full-text styles: advance revealFrac instead of dripText.
        // dripText tracks text so html renders everything.
        if (dripText !== text) dripText = text;
        if (text.length > 0 && revealFrac < 1) {
          if (!draining) draining = true;
          if (chars > 0) {
            revealFrac = Math.min(revealFrac + chars / Math.max(text.length, 1), 1);
          }
        }
        if (revealFrac >= 1) {
          if (draining) draining = false;
          rafActive = false;
          rafId = undefined;
          return;
        }
      } else {
        // Drip-based styles: advance dripText character by character
        if (dripText.length < text.length) {
          if (!draining) draining = true;
          if (chars > 0) {
            dripText = text.slice(0, Math.min(dripText.length + chars, text.length));
          }
          // Decode tail cycles every frame (separate DOM mutation, doesn't
          // re-derive the markdown HTML).
          if (revealStyle === "decode") {
            const remaining = text.length - dripText.length;
            writeScramble(Math.min(DECODE_TAIL_LEN, remaining));
          }
        } else {
          if (draining) draining = false;
          remainder = 0;
          if (revealStyle === "decode") writeScramble(0);
          rafActive = false;
          rafId = undefined;
          return;
        }
      }
      rafId = requestAnimationFrame(loop);
    };
    rafId = requestAnimationFrame(loop);
  }

  // Restart the rAF loop whenever an input that may create new work changes.
  // The loop self-cancels when caught up; this effect re-arms it.
  $effect(() => {
    void text;
    void streaming;
    void revealStyle;
    void useFullText;
    if (workRemaining()) startRaf();
  });

  onMount(() => {
    return () => {
      unmounted = true;
      if (rafId !== undefined) cancelAnimationFrame(rafId);
      rafId = undefined;
      rafActive = false;
    };
  });

  // ── HTML rendering ──
  // html only re-derives when dripText changes (i.e. when chars > 0 advanced
  // it). The decode scramble tail is rendered by direct DOM mutation in a
  // separate sibling span, so it does NOT re-trigger this derivation.
  let html = $derived(dripText ? renderMarkdown(dripText) : "");

  // ── Container CSS class for edge effects ──
  let edgeClass = $derived.by(() => {
    if (!draining) return "";
    switch (revealStyle) {
      case "fade":
        return "drip-edge-fade";
      case "blur":
        return "drip-edge-blur";
      case "scale":
        return "drip-edge-scale";
      case "wipe":
        return "drip-edge-wipe";
      case "cascade":
        return "drip-cascade";
      default:
        return "";
    }
  });

  // ── Cascade: CSS custom property ──
  let cascadeStyle = $derived(
    revealStyle === "cascade" ? `--cascade-pos: ${Math.round(revealFrac * 100)}%` : "",
  );

  // ── Fly-in: wrap words and animate ──
  // Re-runs when html changes (new streaming text) or revealFrac advances
  $effect(() => {
    if (!container || revealStyle !== "fly-in") return;
    void html; // track dependency
    void revealFrac;

    // Collect text nodes (skip code blocks)
    const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT, {
      acceptNode(node) {
        const p = node.parentElement;
        if (!p) return NodeFilter.FILTER_REJECT;
        if (p.closest("pre, code, .code-block")) return NodeFilter.FILTER_REJECT;
        if (p.classList.contains("fly-word")) return NodeFilter.FILTER_REJECT;
        if ((node.textContent ?? "").trim().length === 0) return NodeFilter.FILTER_REJECT;
        return NodeFilter.FILTER_ACCEPT;
      },
    });
    const textNodes: Text[] = [];
    while (walker.nextNode()) textNodes.push(walker.currentNode as Text);

    // Count total words and determine reveal cutoff
    let totalWords = 0;
    for (const node of textNodes) {
      totalWords += (node.textContent ?? "").split(/\s+/).filter(Boolean).length;
    }
    const revealCount = Math.floor(revealFrac * totalWords);

    // Wrap words in spans
    let wordIdx = 0;
    for (const node of textNodes) {
      const txt = node.textContent ?? "";
      const parts = txt.split(/(\s+)/);
      const frag = document.createDocumentFragment();
      for (const part of parts) {
        if (/^\s+$/.test(part)) {
          frag.appendChild(document.createTextNode(part));
        } else if (part.length > 0) {
          const span = document.createElement("span");
          span.textContent = part;
          span.className = "fly-word";
          span.style.display = "inline-block";
          span.style.transition =
            "opacity 0.35s ease-out, transform 0.45s cubic-bezier(0.22,1,0.36,1)";
          if (wordIdx < revealCount) {
            span.style.opacity = "1";
            span.style.transform = "none";
          } else {
            const angle = Math.random() * Math.PI * 2;
            const dist = 25 + Math.random() * 50;
            span.style.opacity = "0";
            span.style.transform = `translate(${Math.cos(angle) * dist}px, ${Math.sin(angle) * dist}px) rotate(${(Math.random() - 0.5) * 15}deg)`;
          }
          frag.appendChild(span);
          wordIdx++;
        }
      }
      node.parentNode?.replaceChild(frag, node);
    }
  });

  // Copy button handlers — only meaningful once streaming completes (no
  // rendered code blocks during drip). Skipping during streaming saves a DOM
  // query + listener churn on every drip frame.
  $effect(() => {
    if (!container || !html || streaming) return;
    const buttons = container.querySelectorAll<HTMLButtonElement>("[data-code-copy]");
    if (buttons.length === 0) return;
    const cleanups: Array<() => void> = [];
    buttons.forEach((btn) => {
      const handler = async () => {
        const codeEl = btn.closest(".code-block")?.querySelector("pre code");
        if (!codeEl) return;
        try {
          await navigator.clipboard.writeText(codeEl.textContent || "");
          btn.textContent = "Copied!";
          btn.classList.add("copied");
          setTimeout(() => {
            btn.textContent = "Copy";
            btn.classList.remove("copied");
          }, 1500);
        } catch {
          /* ignore */
        }
      };
      btn.addEventListener("click", handler);
      cleanups.push(() => btn.removeEventListener("click", handler));
    });
    return () => cleanups.forEach((fn) => fn());
  });

  // Mermaid diagram rendering
  $effect(() => {
    if (!container || !html) return;
    if (streaming) return;
    renderMermaidBlocks(container).catch(() => {});
  });

  // Image lightbox — gate on !streaming so we don't re-walk the DOM on every drip frame.
  let lightboxSrc = $state<string | null>(null);
  $effect(() => {
    if (!container || !html || streaming) return;
    const imgs = container.querySelectorAll<HTMLImageElement>("img");
    if (imgs.length === 0) return;
    const cleanups: Array<() => void> = [];
    imgs.forEach((img) => {
      img.classList.add("md-img-clickable");
      const handler = () => {
        lightboxSrc = img.src;
      };
      img.addEventListener("click", handler);
      cleanups.push(() => img.removeEventListener("click", handler));
    });
    return () => cleanups.forEach((fn) => fn());
  });

  // Resolve relative image paths — same gating; src resolution is irrelevant
  // until the message is fully rendered.
  $effect(() => {
    if (!container || !html || !basePath || streaming) return;
    const imgs = container.querySelectorAll<HTMLImageElement>("img");
    for (const img of imgs) {
      const src = img.getAttribute("src");
      if (!src) continue;
      if (/^(https?:|data:|blob:)/.test(src)) continue;
      if (src.startsWith("/") || /^[a-zA-Z]:/.test(src)) continue;
      const abs = basePath.replace(/\\/g, "/") + "/" + src.replace(/\\/g, "/");
      dbg("markdown", "resolve-img", { src, abs });
      readFileBase64(abs)
        .then(([base64, mime]) => {
          img.src = `data:${mime};base64,${base64}`;
        })
        .catch((e) => {
          dbgWarn("markdown", "img-load-failed", { src, abs, error: e });
        });
    }
  });
</script>

<div
  bind:this={container}
  class="prose prose-sm dark:prose-invert max-w-none
    prose-p:text-foreground prose-p:leading-relaxed
    prose-a:text-primary prose-a:underline prose-a:underline-offset-2
    prose-code:rounded prose-code:bg-muted/70 prose-code:px-1 prose-code:py-0.5 prose-code:text-xs prose-code:font-mono prose-code:before:content-none prose-code:after:content-none
    prose-pre:m-0 prose-pre:p-0 prose-pre:bg-transparent prose-pre:border-0
    prose-li:text-foreground
    {edgeClass} {className}"
  style={cascadeStyle}
>
  {@html html}
  {#if revealStyle === "decode"}
    <span bind:this={scrambleEl} class="decode-scramble"></span>
  {/if}
</div>

<!-- Lightbox -->
{#if lightboxSrc}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="lightbox-overlay" onclick={() => (lightboxSrc = null)}>
    <img src={lightboxSrc} alt="Preview" class="lightbox-img" />
    <div class="lightbox-hint">click to close</div>
  </div>
{/if}
