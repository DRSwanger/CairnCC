<script lang="ts">
  import { listDirectory } from "$lib/api";
  import type { DirEntry } from "$lib/types";
  import { t } from "$lib/i18n/index.svelte";

  let {
    cwd = "/",
    onAddFile,
  }: {
    cwd?: string;
    onAddFile: (path: string) => void;
  } = $props();

  let currentPath = $state(cwd);
  let entries = $state<DirEntry[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let showHidden = $state(false);

  // Track navigation history for back button
  let history = $state<string[]>([]);

  $effect(() => {
    loadDir(cwd);
  });

  async function loadDir(path: string) {
    if (!path) return;
    loading = true;
    error = null;
    try {
      const result = await listDirectory(path, showHidden);
      currentPath = result.path;
      entries = result.entries;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function navigate(entry: DirEntry) {
    if (!entry.is_dir) {
      const fullPath = currentPath.replace(/\/$/, "") + "/" + entry.name;
      onAddFile(fullPath);
      return;
    }
    history = [...history, currentPath];
    const next = currentPath.replace(/\/$/, "") + "/" + entry.name;
    loadDir(next);
  }

  function goBack() {
    if (history.length === 0) return;
    const prev = history[history.length - 1];
    history = history.slice(0, -1);
    loadDir(prev);
  }

  function refresh() {
    loadDir(currentPath);
  }

  function formatSize(bytes: number): string {
    if (bytes < 1024) return bytes + "B";
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + "K";
    return (bytes / (1024 * 1024)).toFixed(1) + "M";
  }

  function shortPath(p: string): string {
    const parts = p.split("/").filter(Boolean);
    return parts.length > 2 ? "…/" + parts.slice(-2).join("/") : p || "/";
  }
</script>

<div class="flex h-full flex-col overflow-hidden">
  <!-- Header -->
  <div class="flex items-center gap-1 px-2 py-1.5 border-b border-border/50">
    <button
      class="p-1 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors disabled:opacity-30"
      onclick={goBack}
      disabled={history.length === 0}
      title="Back"
    >
      <svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="15 18 9 12 15 6" />
      </svg>
    </button>
    <button
      class="p-1 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
      onclick={refresh}
      title="Refresh"
    >
      <svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="23 4 23 10 17 10" />
        <polyline points="1 20 1 14 7 14" />
        <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
      </svg>
    </button>
    <span class="flex-1 text-[10px] text-muted-foreground truncate min-w-0" title={currentPath}>
      {shortPath(currentPath)}
    </span>
    <button
      class="p-1 rounded transition-colors {showHidden ? 'text-foreground bg-accent' : 'text-muted-foreground hover:text-foreground hover:bg-accent/50'}"
      onclick={() => { showHidden = !showHidden; loadDir(currentPath); }}
      title={showHidden ? "Hide hidden files" : "Show hidden files"}
    >
      <svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
        <circle cx="12" cy="12" r="3" />
        {#if !showHidden}
          <line x1="1" y1="1" x2="23" y2="23" />
        {/if}
      </svg>
    </button>
  </div>

  <!-- Content -->
  <div class="flex-1 overflow-y-auto py-0.5">
    {#if loading}
      <div class="flex items-center justify-center h-12 text-xs text-muted-foreground/50">
        Loading…
      </div>
    {:else if error}
      <div class="px-2.5 py-2 text-[11px] text-destructive">
        {error}
      </div>
    {:else if entries.length === 0}
      <div class="flex items-center justify-center h-12 text-xs text-muted-foreground/50">
        Empty folder
      </div>
    {:else}
      {#each entries as entry (entry.name)}
        <button
          class="w-full text-left px-2.5 py-1 hover:bg-accent/50 rounded-sm transition-colors group"
          onclick={() => navigate(entry)}
          title={entry.is_dir ? `Open ${entry.name}` : `Add ${entry.name} to prompt`}
        >
          <div class="flex items-center gap-1.5 min-w-0">
            {#if entry.is_dir}
              <!-- Folder icon -->
              <svg class="h-3.5 w-3.5 shrink-0 text-amber-500/80" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
              </svg>
            {:else}
              <!-- File icon -->
              <svg class="h-3.5 w-3.5 shrink-0 text-muted-foreground/60" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
                <polyline points="14 2 14 8 20 8" />
              </svg>
            {/if}
            <span class="text-[11px] text-foreground truncate min-w-0 group-hover:text-foreground">
              {entry.name}
            </span>
            {#if !entry.is_dir && entry.size > 0}
              <span class="text-[9px] text-muted-foreground/50 shrink-0 ml-auto">
                {formatSize(entry.size)}
              </span>
            {/if}
            {#if entry.is_dir}
              <svg class="h-3 w-3 shrink-0 text-muted-foreground/30 ml-auto" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="9 18 15 12 9 6" />
              </svg>
            {/if}
          </div>
        </button>
      {/each}
    {/if}
  </div>

  <!-- Footer hint -->
  <div class="px-2.5 py-1 border-t border-border/50 text-[9px] text-muted-foreground/40">
    Click file to add path to prompt
  </div>
</div>
