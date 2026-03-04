<script lang="ts">
  import { t } from "$lib/i18n/index.svelte";
  import { fmtTime } from "$lib/i18n/format";
  import * as api from "$lib/api";
  import { dbg, dbgWarn } from "$lib/utils/debug";
  import {
    type RewindCandidate,
    type RewindDryRunResult,
    parseDryRunResult,
    parseExecuteResult,
    isDryRunUnsupported,
  } from "$lib/utils/rewind";
  import Modal from "./Modal.svelte";

  let {
    open = $bindable(false),
    runId = "",
    candidates = [] as RewindCandidate[],
    onSuccess,
  }: {
    open: boolean;
    runId: string;
    candidates: RewindCandidate[];
    onSuccess?: () => void;
  } = $props();

  // ── Internal state ──
  let phase = $state<"select" | "preview" | "executing">("select");
  let selected = $state<RewindCandidate | null>(null);
  let dryRunLoading = $state(false);
  let dryRunResult = $state<RewindDryRunResult | null>(null);
  let dryRunSkipped = $state(false); // true = CLI doesn't support dry_run, allow execute without preview
  let executeError = $state<string | null>(null);
  let requestSeq = $state(0); // race-condition guard: incrementing sequence number

  // ── Reset on close ──
  $effect(() => {
    if (!open) {
      requestSeq++; // invalidate in-flight requests
      phase = "select";
      selected = null;
      dryRunLoading = false;
      dryRunResult = null;
      dryRunSkipped = false;
      executeError = null;
    }
  });

  // ── Select a checkpoint → dryRun preview ──
  async function selectCheckpoint(c: RewindCandidate) {
    const seq = ++requestSeq;
    selected = c;
    phase = "preview";
    dryRunLoading = true;
    dryRunResult = null;
    dryRunSkipped = false;
    executeError = null;
    dbg("rewind-modal", "selectCheckpoint", { uuid: c.cliUuid, seq });

    try {
      const raw = await api.rewindFiles(runId, { userMessageId: c.cliUuid, dryRun: true });
      if (seq !== requestSeq || !open) return; // stale or modal closed
      dbg("rewind-modal", "dryRun response", { raw });

      const result = parseDryRunResult(raw);
      // Check if CLI doesn't support dry_run (resolve path — returns subtype:"error" without throwing)
      if (!result.canRewind && result.error && isDryRunUnsupported(result.error)) {
        dbg("rewind-modal", "dryRun unsupported (resolve path), allowing skip", {
          error: result.error,
        });
        dryRunSkipped = true;
      } else {
        dryRunResult = result;
      }
    } catch (e) {
      if (seq !== requestSeq || !open) return; // stale or modal closed
      // Distinguish "CLI doesn't support dry_run" (exception path) vs hard failure
      if (isDryRunUnsupported(e)) {
        dbg("rewind-modal", "dryRun unsupported (exception path), allowing skip");
        dryRunSkipped = true;
      } else {
        dbgWarn("rewind-modal", "dryRun hard failure", e);
        dryRunResult = { canRewind: false, error: String(e) };
      }
    } finally {
      if (seq === requestSeq) dryRunLoading = false;
    }
  }

  // ── Execute rewind ──
  async function executeRewind() {
    if (!selected) return;
    const seq = ++requestSeq;
    phase = "executing";
    executeError = null;
    dbg("rewind-modal", "executeRewind", { uuid: selected.cliUuid, seq });

    try {
      const raw = await api.rewindFiles(runId, { userMessageId: selected.cliUuid });
      if (seq !== requestSeq || !open) return;
      dbg("rewind-modal", "execute response", { raw });

      const result = parseExecuteResult(raw);
      if (result.canRewind) {
        dbg("rewind-modal", "execute success");
        onSuccess?.();
        open = false;
      } else {
        dbgWarn("rewind-modal", "execute failed", { error: result.error });
        executeError = result.error ?? t("rewind_checkpointUnavailable");
        phase = "preview";
      }
    } catch (e) {
      if (seq !== requestSeq || !open) return;
      dbgWarn("rewind-modal", "execute exception", e);
      executeError = String(e);
      phase = "preview";
    }
  }

  // ── Go back to selection ──
  function goBack() {
    phase = "select";
    selected = null;
    dryRunResult = null;
    dryRunSkipped = false;
    executeError = null;
  }

  function truncateContent(text: string, max = 80): string {
    if (text.length <= max) return text;
    return text.slice(0, max) + "…";
  }
</script>

<Modal bind:open title={t("rewind_modalTitle")} closeable={phase !== "executing"}>
  <!-- Phase: select -->
  {#if phase === "select"}
    {#if candidates.length === 0}
      <div class="flex flex-col items-center gap-2 py-8 text-center text-muted-foreground">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          class="h-10 w-10 opacity-40"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          stroke-width="1.5"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
          />
        </svg>
        <p class="text-sm font-medium">{t("rewind_noCheckpoints")}</p>
        <p class="text-xs opacity-70">{t("rewind_noCheckpointsHint")}</p>
      </div>
    {:else}
      <p class="mb-3 text-sm text-muted-foreground">{t("rewind_selectCheckpoint")}</p>
      <div class="max-h-[50vh] overflow-y-auto">
        {#each candidates as c (c.cliUuid)}
          <button
            type="button"
            class="w-full rounded-md border border-transparent px-3 py-2 text-left transition-colors
              hover:border-border hover:bg-muted/50"
            onclick={() => selectCheckpoint(c)}
          >
            <div class="flex items-baseline justify-between gap-2">
              <span class="min-w-0 flex-1 truncate text-sm">{truncateContent(c.content)}</span>
              <span class="shrink-0 text-xs text-muted-foreground">{fmtTime(c.ts)}</span>
            </div>
          </button>
        {/each}
      </div>
    {/if}

    <!-- Phase: preview -->
  {:else if phase === "preview"}
    {#if dryRunLoading}
      <!-- Loading spinner -->
      <div class="flex items-center justify-center py-12">
        <div
          class="h-6 w-6 animate-spin rounded-full border-2 border-muted-foreground border-t-transparent"
        ></div>
      </div>
    {:else if dryRunResult && dryRunResult.canRewind}
      <!-- Successful dryRun preview -->
      {#if selected}
        <div class="mb-3 rounded-md bg-muted/50 px-3 py-2 text-sm">
          {truncateContent(selected.content)}
        </div>
      {/if}

      {#if executeError}
        <div class="mb-3 rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {executeError}
        </div>
      {/if}

      {#if dryRunResult.filesChanged && dryRunResult.filesChanged.length > 0}
        <p class="mb-2 text-sm text-muted-foreground">{t("rewind_previewDesc")}</p>
        <div class="mb-4 max-h-[30vh] overflow-y-auto rounded-md border bg-muted/30 p-2">
          {#each dryRunResult.filesChanged as file}
            <div class="truncate px-1 py-0.5 font-mono text-xs">{file}</div>
          {/each}
        </div>
      {:else}
        <p class="mb-4 text-sm text-muted-foreground">{t("rewind_noFilesChanged")}</p>
      {/if}

      <div class="flex justify-end gap-2">
        <button
          type="button"
          class="rounded-md border px-3 py-1.5 text-sm transition-colors hover:bg-muted"
          onclick={goBack}
        >
          {t("rewind_back")}
        </button>
        <button
          type="button"
          class="rounded-md bg-primary px-3 py-1.5 text-sm text-primary-foreground transition-colors hover:bg-primary/90"
          onclick={executeRewind}
        >
          {t("rewind_confirm")}
        </button>
      </div>
    {:else if dryRunSkipped}
      <!-- CLI doesn't support dry_run — allow execute without preview -->
      {#if selected}
        <div class="mb-3 rounded-md bg-muted/50 px-3 py-2 text-sm">
          {truncateContent(selected.content)}
        </div>
      {/if}

      {#if executeError}
        <div class="mb-3 rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {executeError}
        </div>
      {/if}

      <p class="mb-4 text-sm text-muted-foreground">{t("rewind_previewUnavailable")}</p>

      <div class="flex justify-end gap-2">
        <button
          type="button"
          class="rounded-md border px-3 py-1.5 text-sm transition-colors hover:bg-muted"
          onclick={goBack}
        >
          {t("rewind_back")}
        </button>
        <button
          type="button"
          class="rounded-md bg-primary px-3 py-1.5 text-sm text-primary-foreground transition-colors hover:bg-primary/90"
          onclick={executeRewind}
        >
          {t("rewind_confirm")}
        </button>
      </div>
    {:else}
      <!-- dryRun failed (hard error or canRewind: false) -->
      <div class="flex flex-col items-center gap-2 py-8 text-center">
        <p class="text-sm text-destructive">
          {dryRunResult?.error ?? t("rewind_checkpointUnavailable")}
        </p>
      </div>

      <div class="flex justify-end">
        <button
          type="button"
          class="rounded-md border px-3 py-1.5 text-sm transition-colors hover:bg-muted"
          onclick={goBack}
        >
          {t("rewind_back")}
        </button>
      </div>
    {/if}

    <!-- Phase: executing -->
  {:else if phase === "executing"}
    <div class="flex flex-col items-center gap-3 py-12">
      <div
        class="h-6 w-6 animate-spin rounded-full border-2 border-muted-foreground border-t-transparent"
      ></div>
      <p class="text-sm text-muted-foreground">{t("rewind_executing")}</p>
    </div>
  {/if}
</Modal>
