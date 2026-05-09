<script lang="ts">
  import { t } from "$lib/i18n/index.svelte";
  import { fmtTime, fmtDateTime } from "$lib/i18n/format";
  import MarkdownContent from "./MarkdownContent.svelte";
  import FileAttachment from "./FileAttachment.svelte";
  import { IMAGE_TYPES } from "$lib/utils/file-types";
  import type { ChatMessage, Attachment } from "$lib/types";
  import type { RevealStyle } from "$lib/stores/reveal-animation.svelte";

  let {
    message,
    attachments,
    thinkingText,
    onRewind,
    streaming = false,
    streamingText = "",
    draining = $bindable(false),
    rate = 35,
    revealStyle = "slide" as RevealStyle,
  }: {
    message: ChatMessage;
    attachments?: Attachment[];
    thinkingText?: string;
    onRewind?: () => void;
    streaming?: boolean;
    streamingText?: string;
    draining?: boolean;
    rate?: number;
    revealStyle?: RevealStyle;
  } = $props();

  function isImage(att: Attachment): boolean {
    return (IMAGE_TYPES as readonly string[]).includes(att.type);
  }

  const isUser = $derived(message.role === "user");

  let hovered = $state(false);
  let copied = $state(false);
  let collapsed = $state(true);
  let thinkingCollapsed = $state(true);

  const lineCount = $derived(message.content.split("\n").length);
  const isLong = $derived(isUser && lineCount > 10);

  function formatTime(ts: string): string {
    const d = new Date(ts);
    if (isNaN(d.getTime())) return "";
    const now = new Date();
    const isToday =
      d.getFullYear() === now.getFullYear() &&
      d.getMonth() === now.getMonth() &&
      d.getDate() === now.getDate();
    return isToday ? fmtTime(d) : fmtDateTime(d);
  }

  function formatFullTime(ts: string): string {
    return fmtDateTime(ts);
  }

  async function copyContent() {
    try {
      await navigator.clipboard.writeText(message.content);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      // Silently fail
    }
  }
</script>

<div
  class="w-full"
  role="group"
  onmouseenter={() => (hovered = true)}
  onmouseleave={() => (hovered = false)}
>
  <div class="chat-content-width py-3">
    <div class="flex {isUser ? 'justify-end' : 'justify-start'}">
      <div class="flex flex-col {isUser ? 'items-end' : 'items-start'} max-w-[85%] min-w-0">
        <!-- Header: avatar + name + actions + timestamp -->
        <div class="mb-1 flex items-center gap-2 px-1 {isUser ? 'flex-row-reverse' : ''}">
          {#if isUser}
            <div class="flex h-5 w-5 items-center justify-center rounded-full bg-primary/15 text-primary">
              <svg
                class="h-3 w-3"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2" />
                <circle cx="12" cy="7" r="4" />
              </svg>
            </div>
            <span class="text-xs font-semibold text-foreground/80">{t("chat_roleYou")}</span>
          {:else}
            <div
              class="flex h-5 w-5 items-center justify-center rounded-full bg-orange-500/15 text-orange-500"
            >
              <svg
                class="h-3 w-3"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <path
                  d="M12 3l1.912 5.813a2 2 0 0 0 1.275 1.275L21 12l-5.813 1.912a2 2 0 0 0-1.275 1.275L12 21l-1.912-5.813a2 2 0 0 0-1.275-1.275L3 12l5.813-1.912a2 2 0 0 0 1.275-1.275L12 3z"
                />
              </svg>
            </div>
            <span class="text-xs font-semibold text-foreground/80">{t("chat_roleClaude")}</span>
          {/if}
          <span class="text-[10px] text-muted-foreground/70" title={formatFullTime(message.timestamp)}>
            {formatTime(message.timestamp)}
          </span>
          {#if onRewind}
            <button
              class="p-0.5 rounded-md text-muted-foreground/50 hover:bg-muted hover:text-foreground transition-all duration-150 {hovered
                ? 'opacity-100'
                : 'opacity-0'}"
              onclick={onRewind}
              title={t("rewind_toHere")}
            >
              <svg
                class="h-3 w-3"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
                <path d="M3 3v5h5" />
              </svg>
            </button>
          {/if}
          <button
            class="p-0.5 rounded-md text-muted-foreground/50 hover:bg-muted hover:text-foreground transition-all duration-150 {hovered ||
            copied
              ? 'opacity-100'
              : 'opacity-0'}"
            onclick={copyContent}
            title={t("chat_copyMessage")}
          >
            {#if copied}
              <svg
                class="h-3 w-3 text-emerald-500"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"><path d="M20 6 9 17l-5-5" /></svg
              >
            {:else}
              <svg
                class="h-3 w-3"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                ><rect width="14" height="14" x="8" y="8" rx="2" /><path
                  d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"
                /></svg
              >
            {/if}
          </button>
        </div>

        <!-- Bubble -->
        <div
          class="chat-bubble px-4 py-2.5 text-sm leading-relaxed text-foreground rounded-2xl {isUser
            ? 'chat-bubble-user rounded-br-md'
            : 'chat-bubble-assistant rounded-bl-md'}"
        >
          {#if isUser}
            {#if attachments && attachments.length > 0}
              <div class="flex flex-wrap gap-2 mb-2">
                {#each attachments as att}
                  {#if isImage(att) && att.contentBase64}
                    <img
                      src="data:{att.type};base64,{att.contentBase64}"
                      alt={att.name}
                      class="max-h-48 max-w-xs rounded-md border border-border object-contain"
                    />
                  {:else}
                    <FileAttachment name={att.name} size={att.size} mimeType={att.type} />
                  {/if}
                {/each}
              </div>
            {/if}
            {#if isLong}
              <p
                class="whitespace-pre-wrap {collapsed ? 'max-h-24 overflow-hidden' : ''}"
                style={collapsed
                  ? "mask-image: linear-gradient(to bottom, black 70%, transparent);"
                  : ""}
              >
                {message.content}
              </p>
              <button
                class="mt-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
                onclick={() => (collapsed = !collapsed)}
              >
                {collapsed
                  ? t("common_showAllLines", { count: String(lineCount) })
                  : t("common_collapse")}
              </button>
            {:else}
              <p class="whitespace-pre-wrap">{message.content}</p>
            {/if}
          {:else}
            <div class="prose-chat">
              <MarkdownContent
                text={streaming ? streamingText : message.content}
                {streaming}
                bind:draining
                {rate}
                {revealStyle}
              />
            </div>
          {/if}
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  /* Globalized so the fallback streaming container in chat/+page.svelte
     (rendered before a real assistant timeline entry exists) can reuse the
     same bubble look. Otherwise live-streaming text shows up flat/unbubbled. */
  :global(.chat-bubble) {
    box-shadow:
      0 1px 0 rgba(255, 255, 255, 0.06) inset,
      0 1px 2px rgba(0, 0, 0, 0.08),
      0 8px 16px -10px rgba(0, 0, 0, 0.18);
  }
  :global(.dark .chat-bubble) {
    box-shadow:
      0 1px 0 rgba(255, 255, 255, 0.04) inset,
      0 1px 2px rgba(0, 0, 0, 0.35),
      0 10px 22px -12px rgba(0, 0, 0, 0.55);
  }
  :global(.chat-bubble-user) {
    background-image: linear-gradient(
      135deg,
      rgba(var(--bubble-user-rgb, 99 102 241) / 0.22),
      rgba(var(--bubble-user-rgb, 99 102 241) / 0.08)
    );
    border: 1px solid rgba(var(--bubble-user-rgb, 99 102 241) / 0.35);
    box-shadow:
      0 1px 0 rgba(255, 255, 255, 0.08) inset,
      0 1px 2px rgba(0, 0, 0, 0.08),
      0 10px 18px -10px rgba(var(--bubble-user-rgb, 99 102 241) / 0.28);
  }
  :global(.dark .chat-bubble-user) {
    background-image: linear-gradient(
      135deg,
      rgba(var(--bubble-user-rgb, 99 102 241) / 0.32),
      rgba(var(--bubble-user-rgb, 99 102 241) / 0.14)
    );
    border-color: rgba(var(--bubble-user-rgb, 99 102 241) / 0.45);
    box-shadow:
      0 1px 0 rgba(255, 255, 255, 0.06) inset,
      0 1px 2px rgba(0, 0, 0, 0.4),
      0 14px 24px -12px rgba(var(--bubble-user-rgb, 99 102 241) / 0.45);
  }
  :global(.chat-bubble-assistant) {
    background-image: linear-gradient(
      135deg,
      rgba(var(--bubble-asst-rgb, 249 115 22) / 0.18),
      rgba(var(--bubble-asst-rgb, 249 115 22) / 0.06)
    );
    border: 1px solid rgba(var(--bubble-asst-rgb, 249 115 22) / 0.3);
    box-shadow:
      0 1px 0 rgba(255, 255, 255, 0.06) inset,
      0 1px 2px rgba(0, 0, 0, 0.08),
      0 10px 18px -10px rgba(var(--bubble-asst-rgb, 249 115 22) / 0.22);
  }
  :global(.dark .chat-bubble-assistant) {
    background-image: linear-gradient(
      135deg,
      rgba(var(--bubble-asst-rgb, 249 115 22) / 0.26),
      rgba(var(--bubble-asst-rgb, 249 115 22) / 0.1)
    );
    border-color: rgba(var(--bubble-asst-rgb, 249 115 22) / 0.4);
    box-shadow:
      0 1px 0 rgba(255, 255, 255, 0.04) inset,
      0 1px 2px rgba(0, 0, 0, 0.4),
      0 14px 24px -12px rgba(var(--bubble-asst-rgb, 249 115 22) / 0.4);
  }
</style>
