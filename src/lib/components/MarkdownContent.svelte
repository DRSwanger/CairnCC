<script lang="ts">
  import { onMount } from "svelte";
  import { renderMarkdown, renderMermaidBlocks } from "$lib/utils/markdown";
  import { readFileBase64 } from "$lib/api";
  import { dbg, dbgWarn } from "$lib/utils/debug";

  let {
    text = "",
    streaming = false,
    basePath = "",
    class: className = "",
  }: {
    text?: string;
    streaming?: boolean;
    basePath?: string;
    class?: string;
  } = $props();

  let container: HTMLDivElement | undefined = $state();

  // Drip text for streaming: one persistent rAF loop advances dripText by DRIP_CHARS
  // per frame toward the latest text prop. Never restarts on token arrival — avoids
  // the cancel/restart jitter of the $effect approach.
  const DRIP_CHARS = 3;
  let dripText = $state(text);

  onMount(() => {
    let rafId: number;
    function loop() {
      if (streaming && dripText.length < text.length) {
        dripText = text.slice(0, Math.min(dripText.length + DRIP_CHARS, text.length));
      }
      rafId = requestAnimationFrame(loop);
    }
    rafId = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(rafId);
  });

  // When streaming ends, snap immediately to full text
  $effect(() => {
    if (!streaming) dripText = text;
  });

  // Single render path: $derived ensures renderMarkdown is called exactly once per dripText change
  let html = $derived(dripText ? renderMarkdown(dripText) : "");

  // Copy button handlers
  $effect(() => {
    if (!container || !html) return;

    const buttons = container.querySelectorAll<HTMLButtonElement>("[data-code-copy]");
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
          // Silently fail
        }
      };
      btn.addEventListener("click", handler);
      cleanups.push(() => btn.removeEventListener("click", handler));
    });

    return () => {
      cleanups.forEach((fn) => fn());
    };
  });

  // Mermaid diagram rendering
  $effect(() => {
    if (!container || !html) return;
    // Only render when not streaming (avoid half-rendered diagrams)
    if (streaming) return;
    renderMermaidBlocks(container).catch(() => {});
  });

  // Image lightbox
  let lightboxSrc = $state<string | null>(null);
  $effect(() => {
    if (!container || !html) return;
    const imgs = container.querySelectorAll<HTMLImageElement>("img");
    const cleanups: Array<() => void> = [];
    imgs.forEach((img) => {
      img.classList.add("md-img-clickable");
      const handler = () => { lightboxSrc = img.src; };
      img.addEventListener("click", handler);
      cleanups.push(() => img.removeEventListener("click", handler));
    });
    return () => cleanups.forEach((fn) => fn());
  });

  // Resolve relative image paths against basePath (for Explorer file preview)
  $effect(() => {
    if (!container || !html || !basePath) return;

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
    {className}"
>
  {@html html}
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
