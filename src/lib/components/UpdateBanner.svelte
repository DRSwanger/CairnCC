<script lang="ts">
  import { check } from "@tauri-apps/plugin-updater";
  import type { Update } from "@tauri-apps/plugin-updater";
  import { dbg, dbgWarn } from "$lib/utils/debug";
  import { t } from "$lib/i18n/index.svelte";
  import { onMount } from "svelte";
  import { IS_MAC } from "$lib/utils/platform";

  const RELEASES_URL = "https://github.com/DRSwanger/CairnCC/releases";

  let hasUpdate = $state(false);
  let latestVersion = $state("");
  let installing = $state(false);
  let installProgress = $state(0);
  let restartNeeded = $state(false);
  let installError = $state("");
  let update = $state<Update | null>(null);
  let showMacHelp = $state(false);

  function isDismissed(version: string): boolean {
    return sessionStorage.getItem(`ocv:update-dismissed:${version}`) === "1";
  }

  function dismiss() {
    dbg("update-banner", "dismissed", latestVersion);
    sessionStorage.setItem(`ocv:update-dismissed:${latestVersion}`, "1");
    hasUpdate = false;
  }

  async function openReleasesPage() {
    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(RELEASES_URL);
    } catch {
      window.open(RELEASES_URL, "_blank");
    }
    showMacHelp = true;
  }

  async function downloadAndInstall() {
    if (!update || installing) return;
    installing = true;
    installProgress = 0;
    installError = "";
    let totalDownloaded = 0;
    let contentLength: number | undefined;
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          contentLength = event.data.contentLength;
          totalDownloaded = 0;
        } else if (event.event === "Progress") {
          totalDownloaded += event.data.chunkLength;
          if (contentLength) {
            installProgress = Math.round((totalDownloaded / contentLength) * 100);
          }
        } else if (event.event === "Finished") {
          installProgress = 100;
        }
      });
      restartNeeded = true;
    } catch (e) {
      dbgWarn("update-banner", "install failed", e);
      installError = String(e);
    } finally {
      installing = false;
    }
  }

  async function copyXattrCommand() {
    try {
      await navigator.clipboard.writeText("xattr -cr /Applications/CairnCC.app");
    } catch { /* ignore */ }
  }

  onMount(() => {
    const timerId = setTimeout(async () => {
      try {
        const result = await check();
        dbg("update-banner", "check result", result);
        if (result && !isDismissed(result.version)) {
          hasUpdate = true;
          latestVersion = result.version;
          update = result;
        }
      } catch (e) {
        dbgWarn("update-banner", "check failed", e);
      }
    }, 3000);
    return () => clearTimeout(timerId);
  });
</script>

{#if hasUpdate}
  <div
    class="border-b border-primary/30 bg-primary/10 px-4 py-1.5 text-sm"
  >
    <div class="flex items-center justify-between gap-2">
      {#if restartNeeded}
        <span class="text-xs text-foreground">
          Update installed — restart CairnCC to apply v{latestVersion}
        </span>
      {:else if installing}
        <span class="flex items-center gap-2 text-xs text-foreground">
          <span
            class="h-3 w-3 animate-spin rounded-full border border-foreground/30 border-t-foreground"
          ></span>
          Installing v{latestVersion}…{installProgress > 0 ? ` (${installProgress}%)` : ""}
        </span>
      {:else}
        <span class="text-foreground">
          {t("appUpdate_available", { version: latestVersion })}
        </span>
      {/if}
      <div class="flex items-center gap-2">
        {#if installError}
          <span class="text-xs text-destructive">{installError}</span>
        {/if}
        {#if !installing && !restartNeeded}
          {#if IS_MAC}
            <button
              class="rounded-md bg-primary px-3 py-0.5 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90"
              onclick={openReleasesPage}
            >
              Download from GitHub
            </button>
          {:else}
            <button
              class="rounded-md bg-primary px-3 py-0.5 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90"
              onclick={downloadAndInstall}
            >
              {t("appUpdate_download")}
            </button>
          {/if}
        {/if}
        {#if !installing}
          <button
            class="rounded-md px-2 py-0.5 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            onclick={dismiss}
            title={t("appUpdate_dismiss")}
          >
            <svg
              class="h-3.5 w-3.5"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"><path d="M18 6 6 18" /><path d="m6 6 12 12" /></svg
            >
          </button>
        {/if}
      </div>
    </div>
    {#if IS_MAC && showMacHelp}
      <div class="mt-2 rounded-md bg-muted/60 border border-border/40 px-3 py-2 text-xs text-muted-foreground">
        <p class="font-medium text-foreground mb-1">After downloading the .dmg and dragging CairnCC to Applications:</p>
        <p class="mb-1.5">macOS may block the app because it isn't signed with an Apple Developer license. Run this in Terminal to allow it:</p>
        <div class="flex items-center gap-2">
          <code class="flex-1 rounded bg-background/80 px-2 py-1 font-mono text-[11px] text-foreground select-all">xattr -cr /Applications/CairnCC.app</code>
          <button
            class="shrink-0 rounded px-2 py-1 text-[11px] text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
            onclick={copyXattrCommand}
            title="Copy command"
          >
            <svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <rect width="14" height="14" x="8" y="8" rx="2" /><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" />
            </svg>
          </button>
        </div>
        <p class="mt-1.5">Then open CairnCC normally from Applications.</p>
      </div>
    {/if}
  </div>
{/if}
