<script lang="ts">
  import {
    checkAgentCli,
    checkAuthStatus,
    detectInstallMethods,
    runClaudeLogin,
    updateUserSettings,
    checkSshKey,
    generateSshKey,
    testRemoteHost,
  } from "$lib/api";
  import type { InstallMethod, PlatformPreset, RemoteHost, RemoteTestResult, SshKeyInfo } from "$lib/types";
  import { PLATFORM_PRESETS, PRESET_CATEGORIES } from "$lib/utils/platform-presets";
  import { dbg, dbgWarn } from "$lib/utils/debug";
  import { IS_WINDOWS } from "$lib/utils/platform";
  import { getTransport } from "$lib/transport";
  import { t } from "$lib/i18n/index.svelte";

  let { onComplete }: { onComplete: () => void } = $props();

  type WizardStep =
    | "checking"
    | "connection_type"
    | "cli_not_found"
    | "auth_choice"
    | "oauth_login"
    | "api_key_setup"
    | "remote_setup"
    | "done";

  let step = $state<WizardStep>("checking");
  let error = $state("");

  // CLI install state
  let installMethods = $state<InstallMethod[]>([]);

  // Copy button state: method id → "copy" | "copied"
  let copyStates = $state<Record<string, string>>({});

  // Recheck state
  let rechecking = $state(false);

  // OAuth state
  let oauthLoading = $state(false);
  let installProgress = $state<string[]>([]);

  // API key state
  let selectedPlatform = $state<PlatformPreset | null>(null);
  let apiKey = $state("");
  let customBaseUrl = $state("");
  let showKey = $state(false);
  let saving = $state(false);

  // Done state
  let doneTimer = $state<ReturnType<typeof setTimeout> | null>(null);

  // ── Remote host form state ──
  let remoteFormName = $state("My Server");
  let remoteFormHost = $state("");
  let remoteFormUser = $state("");
  let remoteFormPort = $state(22);
  let remoteFormKeyPath = $state("");
  let remoteFormTouched = $state(false);
  let remoteTestResult = $state<RemoteTestResult | null>(null);
  let remoteTesting = $state(false);
  let remoteSaving = $state(false);
  let remoteError = $state("");

  // ── SSH key mini-wizard ──
  type SshKeyStep = "idle" | "checking" | "no_key" | "has_key" | "pub_missing" | "generating" | "done" | "error";
  let sshKeyStep = $state<SshKeyStep>("idle");
  let sshKeyInfo = $state<SshKeyInfo | null>(null);
  let sshKeyError = $state("");
  let sshCopied = $state(false);

  function shellQuote(s: string): string {
    return "'" + s.replace(/'/g, "'\\''") + "'";
  }
  function pwshQuote(s: string): string {
    return "'" + s.replace(/'/g, "''") + "'";
  }
  function buildCopyCommand(keyInfo: SshKeyInfo): string {
    const host = remoteFormHost.trim();
    const user = remoteFormUser.trim();
    const port = remoteFormPort || 22;
    if (IS_WINDOWS) {
      const pubPath = pwshQuote(keyInfo.key_path_expanded + ".pub");
      const target = pwshQuote(`${user}@${host}`);
      const remoteScript = pwshQuote(
        "mkdir -p ~/.ssh && chmod 700 ~/.ssh && " +
          "touch ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys && " +
          'key=$(cat) && (grep -qxF "$key" ~/.ssh/authorized_keys 2>/dev/null || ' +
          'echo "$key" >> ~/.ssh/authorized_keys)'
      );
      return `Get-Content -LiteralPath ${pubPath} -Raw | ssh -p ${port} ${target} ${remoteScript}`;
    }
    const keyArg = shellQuote(keyInfo.key_path_expanded);
    const pubArg = shellQuote(keyInfo.key_path_expanded + ".pub");
    const target = `${shellQuote(user)}@${shellQuote(host)}`;
    if (keyInfo.ssh_copy_id_available) {
      return `ssh-copy-id -i ${keyArg} -p ${port} ${target}`;
    }
    const rs = "mkdir -p ~/.ssh && chmod 700 ~/.ssh && touch ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys && " +
      'key=$(cat) && (grep -qxF "$key" ~/.ssh/authorized_keys 2>/dev/null || echo "$key" >> ~/.ssh/authorized_keys)';
    return `cat ${pubArg} | ssh -p ${port} ${target} ${shellQuote(rs)}`;
  }

  async function startSshKeySetup() {
    sshKeyStep = "checking";
    sshKeyError = "";
    sshCopied = false;
    try {
      const info = await checkSshKey();
      sshKeyInfo = info;
      if (info.exists && info.pub_exists) sshKeyStep = "has_key";
      else if (info.exists && !info.pub_exists) sshKeyStep = "pub_missing";
      else sshKeyStep = "no_key";
    } catch (e) {
      sshKeyError = String(e);
      sshKeyStep = "error";
    }
  }

  async function doGenerateSshKey() {
    sshKeyStep = "generating";
    sshKeyError = "";
    try {
      const info = await generateSshKey();
      sshKeyInfo = info;
      sshKeyStep = "has_key";
    } catch (e) {
      sshKeyError = String(e);
      sshKeyStep = "error";
    }
  }

  async function testRemoteConnection() {
    if (!remoteFormHost.trim() || !remoteFormUser.trim()) {
      remoteFormTouched = true;
      return;
    }
    remoteTesting = true;
    remoteError = "";
    remoteTestResult = null;
    try {
      const keyPath = (sshKeyInfo?.key_path ?? remoteFormKeyPath).trim() || undefined;
      const result = await testRemoteHost(
        remoteFormHost.trim(),
        remoteFormUser.trim(),
        remoteFormPort || undefined,
        keyPath,
        undefined,
      );
      remoteTestResult = result;
      if (!result.ssh_ok) remoteError = result.error ?? "SSH connection failed";
    } catch (e) {
      remoteError = String(e);
    } finally {
      remoteTesting = false;
    }
  }

  async function saveRemoteAndComplete() {
    remoteSaving = true;
    remoteError = "";
    try {
      const newHost: RemoteHost = {
        name: remoteFormName.trim() || "My Server",
        host: remoteFormHost.trim(),
        user: remoteFormUser.trim(),
        port: remoteFormPort || 22,
        key_path: (sshKeyInfo?.key_path ?? remoteFormKeyPath).trim() || undefined,
        forward_api_key: true,
      };
      await updateUserSettings({ remote_hosts: [newHost] });
      await completeOnboarding();
    } catch (e) {
      remoteError = String(e);
    } finally {
      remoteSaving = false;
    }
  }

  // Start checking on mount
  $effect(() => {
    if (step === "checking") {
      runInitialCheck();
    }
  });

  async function runInitialCheck() {
    dbg("wizard", "starting initial check");
    try {
      const [cliResult, authResult] = await Promise.all([
        checkAgentCli("claude"),
        checkAuthStatus(),
      ]);

      dbg("wizard", "check results", {
        cliFound: cliResult.found,
        hasOAuth: authResult.has_oauth,
        hasApiKey: authResult.has_api_key,
      });

      if (cliResult.found && (authResult.has_oauth || authResult.has_api_key)) {
        // Fully configured — mark onboarding done and skip
        await completeOnboarding();
        return;
      }

      if (cliResult.found && !authResult.has_oauth && !authResult.has_api_key) {
        // CLI found but no auth — go to auth choice
        step = "auth_choice";
        return;
      }

      // CLI not found — let user pick local or remote
      step = "connection_type";
      await loadInstallMethods();
    } catch (e) {
      dbgWarn("wizard", "initial check error", e);
      // If check fails, let user pick local or remote
      step = "connection_type";
      await loadInstallMethods();
    }
  }

  async function loadInstallMethods() {
    try {
      installMethods = await detectInstallMethods();
      dbg("wizard", "install methods", installMethods);
    } catch (e) {
      dbgWarn("wizard", "detect methods error", e);
      installMethods = [];
    }
  }

  async function copyCommand(method: InstallMethod) {
    try {
      await navigator.clipboard.writeText(method.command);
      copyStates = { ...copyStates, [method.id]: "copied" };
      setTimeout(() => {
        copyStates = { ...copyStates, [method.id]: "copy" };
      }, 1500);
    } catch (e) {
      dbgWarn("wizard", "copy failed", e);
    }
  }

  async function recheckCli() {
    rechecking = true;
    try {
      const result = await checkAgentCli("claude");
      dbg("wizard", "recheck result", result);
      if (result.found) {
        step = "auth_choice";
      }
    } catch (e) {
      dbgWarn("wizard", "recheck error", e);
    } finally {
      rechecking = false;
    }
  }

  async function startOAuthLogin() {
    step = "oauth_login";
    oauthLoading = true;
    error = "";
    installProgress = [];

    const wizTransport = getTransport();
    const unlistenAppend = await wizTransport.listen<string>("setup-progress", (payload) => {
      installProgress = [...installProgress, payload];
    });
    const unlistenReplace = await wizTransport.listen<string>(
      "setup-progress-replace",
      (payload) => {
        if (installProgress.length > 0) {
          installProgress = [...installProgress.slice(0, -1), payload];
        } else {
          installProgress = [payload];
        }
      },
    );
    const unlisten = () => {
      unlistenAppend();
      unlistenReplace();
    };

    try {
      const success = await runClaudeLogin();
      unlisten();

      if (success) {
        dbg("wizard", "oauth login success");
        await completeOnboarding();
      } else {
        error = t("setup_loginFailed");
      }
    } catch (e) {
      unlisten();
      dbgWarn("wizard", "oauth login error", e);
      error = String(e);
    } finally {
      oauthLoading = false;
    }
  }

  function selectPlatform(preset: PlatformPreset) {
    selectedPlatform = preset;
    apiKey = "";
    customBaseUrl = preset.base_url;
    showKey = false;
  }

  async function saveApiKey() {
    if (!selectedPlatform) return;
    saving = true;
    error = "";

    try {
      const effectiveBaseUrl =
        selectedPlatform.id === "custom" ? customBaseUrl : selectedPlatform.base_url;

      await updateUserSettings({
        auth_mode: "api",
        anthropic_api_key: apiKey || undefined,
        anthropic_base_url: effectiveBaseUrl || undefined,
        auth_env_var: selectedPlatform.auth_env_var,
        onboarding_completed: true,
      });

      dbg("wizard", "api key saved", {
        platform: selectedPlatform.id,
        hasKey: !!apiKey,
        hasBaseUrl: !!effectiveBaseUrl,
      });

      await completeOnboarding();
    } catch (e) {
      dbgWarn("wizard", "save api key error", e);
      error = String(e);
    } finally {
      saving = false;
    }
  }

  async function completeOnboarding() {
    try {
      await updateUserSettings({ onboarding_completed: true });
    } catch {
      // Non-critical — continue anyway
    }
    step = "done";
    doneTimer = setTimeout(() => {
      onComplete();
    }, 2000);
  }

  function finishNow() {
    if (doneTimer) clearTimeout(doneTimer);
    onComplete();
  }

  let availableMethods = $derived(installMethods.filter((m) => m.available));
  let unavailableMethods = $derived(installMethods.filter((m) => !m.available));
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center bg-background">
  <div class="w-full max-w-xl mx-auto px-6">
    {#if step === "checking"}
      <!-- Checking step -->
      <div class="flex flex-col items-center gap-4 py-16">
        <div
          class="h-8 w-8 border-2 border-primary/30 border-t-primary rounded-full animate-spin"
        ></div>
        <p class="text-sm text-muted-foreground">{t("setup_checking")}</p>
      </div>
    {:else if step === "connection_type"}
      <!-- Pick: local install or remote server -->
      <div class="flex flex-col gap-6">
        <div class="text-center">
          <h2 class="text-xl font-semibold">How would you like to connect?</h2>
          <p class="text-sm text-muted-foreground mt-2">
            Claude Code runs the AI — choose where it lives.
          </p>
        </div>

        <div class="grid grid-cols-2 gap-4">
          <!-- Local -->
          <button
            class="flex flex-col items-start gap-3 rounded-xl border border-border p-5 text-left transition-colors hover:border-primary/50 hover:bg-accent/40"
            onclick={() => { step = "cli_not_found"; }}
          >
            <div class="flex h-10 w-10 items-center justify-center rounded-lg bg-muted">
              <svg class="h-5 w-5 text-foreground" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                <rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/>
              </svg>
            </div>
            <div>
              <p class="font-semibold text-sm">This Machine</p>
              <p class="text-xs text-muted-foreground mt-1">Install Claude Code locally — everything runs on this computer.</p>
            </div>
          </button>

          <!-- Remote -->
          <button
            class="flex flex-col items-start gap-3 rounded-xl border border-border p-5 text-left transition-colors hover:border-primary/50 hover:bg-accent/40"
            onclick={() => { step = "remote_setup"; }}
          >
            <div class="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10">
              <svg class="h-5 w-5 text-primary" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                <path d="M5 12.55a11 11 0 0 1 14.08 0"/><path d="M1.42 9a16 16 0 0 1 21.16 0"/><path d="M8.53 16.11a6 6 0 0 1 6.95 0"/><circle cx="12" cy="20" r="1" fill="currentColor"/>
              </svg>
            </div>
            <div>
              <p class="font-semibold text-sm">Remote Server</p>
              <p class="text-xs text-muted-foreground mt-1">Connect via SSH to a Linux or Mac machine where Claude Code is already set up.</p>
            </div>
            <span class="rounded-full bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary">Recommended</span>
          </button>
        </div>

        <p class="text-xs text-muted-foreground text-center">
          You can change this any time in Settings → Remote Hosts.
        </p>
      </div>

    {:else if step === "remote_setup"}
      <!-- Remote server SSH setup -->
      <div class="flex flex-col gap-5 max-h-[85vh] overflow-y-auto pr-1">
        <!-- Header -->
        <div class="flex items-center gap-2">
          <button
            class="rounded-md p-1 hover:bg-accent transition-colors"
            onclick={() => { step = "connection_type"; remoteTestResult = null; remoteError = ""; sshKeyStep = "idle"; }}
            aria-label="Back"
          >
            <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m15 18-6-6 6-6"/></svg>
          </button>
          <h2 class="text-lg font-semibold">Connect to Remote Server</h2>
        </div>

        <!-- Connection fields -->
        <div class="flex flex-col gap-3">
          <div class="grid grid-cols-3 gap-3">
            <div class="col-span-2 flex flex-col gap-1.5">
              <label for="remote-host" class="text-xs font-medium text-muted-foreground">Host / IP Address</label>
              <input
                id="remote-host"
                type="text"
                bind:value={remoteFormHost}
                placeholder="192.168.1.100"
                class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm font-mono focus:outline-none focus:border-ring {remoteFormTouched && !remoteFormHost.trim() ? 'border-destructive' : ''}"
              />
            </div>
            <div class="flex flex-col gap-1.5">
              <label for="remote-port" class="text-xs font-medium text-muted-foreground">Port</label>
              <input
                id="remote-port"
                type="number"
                bind:value={remoteFormPort}
                placeholder="22"
                class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm font-mono focus:outline-none focus:border-ring"
              />
            </div>
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div class="flex flex-col gap-1.5">
              <label for="remote-user" class="text-xs font-medium text-muted-foreground">Username</label>
              <input
                id="remote-user"
                type="text"
                bind:value={remoteFormUser}
                placeholder="dallas"
                class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm font-mono focus:outline-none focus:border-ring {remoteFormTouched && !remoteFormUser.trim() ? 'border-destructive' : ''}"
              />
            </div>
            <div class="flex flex-col gap-1.5">
              <label for="remote-name" class="text-xs font-medium text-muted-foreground">Nickname (optional)</label>
              <input
                id="remote-name"
                type="text"
                bind:value={remoteFormName}
                placeholder="My Server"
                class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm focus:outline-none focus:border-ring"
              />
            </div>
          </div>
        </div>

        <!-- SSH Key section -->
        <div class="rounded-lg border border-border bg-muted/20 p-4 flex flex-col gap-3">
          <div class="flex items-center justify-between">
            <div>
              <p class="text-sm font-medium">SSH Key Authentication</p>
              <p class="text-xs text-muted-foreground mt-0.5">Needed to connect without a password.</p>
            </div>
            {#if sshKeyStep === "idle"}
              <button
                class="rounded-md border border-border px-3 py-1.5 text-xs font-medium hover:bg-accent transition-colors"
                onclick={startSshKeySetup}
              >Set up SSH Key</button>
            {/if}
          </div>

          {#if sshKeyStep === "checking" || sshKeyStep === "generating"}
            <div class="flex items-center gap-2 text-xs text-muted-foreground">
              <span class="h-3 w-3 border border-foreground/30 border-t-foreground rounded-full animate-spin"></span>
              {sshKeyStep === "generating" ? "Generating key…" : "Checking for SSH keys…"}
            </div>

          {:else if sshKeyStep === "no_key"}
            <div class="flex flex-col gap-2">
              <p class="text-xs text-muted-foreground">No SSH key found. Generate one now:</p>
              <button
                class="self-start rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground hover:bg-primary/90 transition-colors"
                onclick={doGenerateSshKey}
              >Generate SSH Key</button>
            </div>

          {:else if sshKeyStep === "has_key" || sshKeyStep === "pub_missing"}
            <div class="flex flex-col gap-2">
              {#if sshKeyStep === "pub_missing"}
                <p class="text-xs text-amber-500">Private key found but public key is missing — key may not work.</p>
              {:else}
                <div class="flex items-center gap-1.5 text-xs text-green-500">
                  <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>
                  SSH key ready — {sshKeyInfo?.key_type ?? "RSA"} key at <code class="font-mono">{sshKeyInfo?.key_path}</code>
                </div>
              {/if}
              {#if remoteFormHost.trim() && remoteFormUser.trim() && sshKeyInfo}
                <div class="flex flex-col gap-1.5">
                  <p class="text-xs text-muted-foreground">Run this on <strong>this machine</strong> to authorize the key on your server{IS_WINDOWS ? ' — paste into <strong>PowerShell</strong>, not Command Prompt' : ''}:</p>
                  <div class="flex items-center gap-2 rounded-md bg-background border border-border p-2">
                    <code class="flex-1 text-[10px] font-mono text-foreground/80 break-all select-all">{buildCopyCommand(sshKeyInfo)}</code>
                    <button
                      class="shrink-0 rounded px-2 py-0.5 text-xs border border-border {sshCopied ? 'text-green-500 border-green-500/30' : 'text-muted-foreground'} hover:text-foreground transition-colors"
                      onclick={async () => { await navigator.clipboard.writeText(buildCopyCommand(sshKeyInfo!)); sshCopied = true; setTimeout(() => sshCopied = false, 1500); }}
                    >{sshCopied ? "Copied!" : "Copy"}</button>
                  </div>
                  <p class="text-[10px] text-muted-foreground">After running that command, click "Test Connection" below.</p>
                </div>
              {:else}
                <p class="text-xs text-muted-foreground">Fill in the host and username above to get the setup command.</p>
              {/if}
            </div>

          {:else if sshKeyStep === "done"}
            <div class="flex items-center gap-1.5 text-xs text-green-500">
              <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>
              SSH verified and connected.
            </div>

          {:else if sshKeyStep === "error"}
            <p class="text-xs text-destructive">{sshKeyError}</p>
          {/if}
        </div>

        <!-- Test result -->
        {#if remoteTestResult}
          <div class="rounded-lg border p-3 flex flex-col gap-1.5 {remoteTestResult.ssh_ok ? 'border-green-500/30 bg-green-500/5' : 'border-destructive/30 bg-destructive/5'}">
            <div class="flex items-center gap-2 text-xs font-medium {remoteTestResult.ssh_ok ? 'text-green-500' : 'text-destructive'}">
              {#if remoteTestResult.ssh_ok}
                <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>
                SSH connected
              {:else}
                <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18M6 6l12 12"/></svg>
                Connection failed
              {/if}
            </div>
            {#if remoteTestResult.ssh_ok && remoteTestResult.cli_found}
              <p class="text-xs text-muted-foreground">Claude Code found: <code class="font-mono">{remoteTestResult.cli_version ?? "unknown version"}</code></p>
            {:else if remoteTestResult.ssh_ok && !remoteTestResult.cli_found}
              <p class="text-xs text-amber-500">SSH works but Claude Code wasn't found on the server. Make sure it's installed there.</p>
            {/if}
            {#if remoteTestResult.error && !remoteTestResult.ssh_ok}
              <p class="text-xs text-muted-foreground">{remoteTestResult.error}</p>
            {/if}
          </div>
        {/if}

        {#if remoteError && !remoteTestResult}
          <p class="text-xs text-destructive">{remoteError}</p>
        {/if}

        <!-- Action buttons -->
        <div class="flex items-center gap-3">
          <button
            class="flex-1 rounded-md border border-border px-4 py-2 text-sm hover:bg-accent transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
            disabled={remoteTesting || !remoteFormHost.trim() || !remoteFormUser.trim()}
            onclick={testRemoteConnection}
          >
            {#if remoteTesting}
              <span class="h-3 w-3 border border-foreground/30 border-t-foreground rounded-full animate-spin"></span>
              Testing…
            {:else}
              Test Connection
            {/if}
          </button>
          <button
            class="flex-1 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
            disabled={remoteSaving || !remoteTestResult?.ssh_ok}
            onclick={saveRemoteAndComplete}
          >
            {#if remoteSaving}
              <span class="h-3 w-3 border border-primary-foreground/30 border-t-primary-foreground rounded-full animate-spin"></span>
              Saving…
            {:else}
              Save & Launch
            {/if}
          </button>
        </div>

        <p class="text-xs text-muted-foreground text-center">
          "Save & Launch" becomes available after a successful connection test.
        </p>
      </div>

    {:else if step === "cli_not_found"}
      <!-- CLI not found — show install commands to copy -->
      <div class="flex flex-col gap-6">
        <div class="text-center">
          <h2 class="text-xl font-semibold">{t("setup_cliNotFound")}</h2>
          <p class="text-sm text-muted-foreground mt-2">{t("setup_cliNotFoundDesc")}</p>
        </div>

        <!-- Available methods — command cards with copy buttons -->
        {#if availableMethods.length > 0}
          <div class="flex flex-col gap-3">
            {#each availableMethods as method, i}
              <div class="rounded-lg border border-border p-3 bg-muted/30">
                <div class="flex items-center gap-3">
                  <code class="flex-1 text-sm font-mono text-foreground/90 select-all"
                    >$ {method.command}</code
                  >
                  {#if i === 0}
                    <span
                      class="rounded-full bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary whitespace-nowrap"
                      >{t("setup_recommended")}</span
                    >
                  {/if}
                  <button
                    class="rounded-md border border-border px-2.5 py-1 text-xs hover:bg-accent transition-colors whitespace-nowrap {copyStates[
                      method.id
                    ] === 'copied'
                      ? 'text-green-600 border-green-500/30'
                      : 'text-muted-foreground'}"
                    onclick={() => copyCommand(method)}
                  >
                    {copyStates[method.id] === "copied"
                      ? t("setup_copied")
                      : t("setup_copyCommand")}
                  </button>
                </div>
                {#if method.note}
                  <p class="text-[10px] text-muted-foreground/70 mt-1">{method.note}</p>
                {/if}
              </div>
            {/each}
          </div>
        {/if}

        <!-- Unavailable methods (greyed out with reason) -->
        {#if unavailableMethods.length > 0}
          <div class="flex flex-col gap-2 opacity-50">
            {#each unavailableMethods as method}
              <div class="flex items-center gap-3 rounded-lg border border-border/50 p-3">
                <span class="text-sm text-muted-foreground">{method.name}</span>
                {#if method.unavailable_reason}
                  <span class="text-xs text-muted-foreground/70">— {method.unavailable_reason}</span
                  >
                {/if}
              </div>
            {/each}
          </div>
        {/if}

        <!-- Action buttons -->
        <div class="flex items-center justify-center gap-3">
          <button
            class="rounded-md border border-border px-4 py-2 text-sm hover:bg-accent transition-colors disabled:opacity-50"
            disabled={rechecking}
            onclick={recheckCli}
          >
            {#if rechecking}
              <span class="flex items-center gap-2">
                <span
                  class="h-3 w-3 border border-foreground/30 border-t-foreground rounded-full animate-spin"
                ></span>
                {t("setup_recheck")}
              </span>
            {:else}
              {t("setup_recheck")}
            {/if}
          </button>
          <button
            class="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors"
            onclick={() => {
              step = "api_key_setup";
            }}
          >
            {t("setup_skipCli")}
          </button>
        </div>

        <!-- Setup hint -->
        <p class="text-xs text-muted-foreground text-center">
          {IS_WINDOWS ? t("setup_winRecheckHint") : t("setup_setupHint")}
        </p>
      </div>
    {:else if step === "auth_choice"}
      <!-- Auth method choice -->
      <div class="flex flex-col gap-6">
        <div class="text-center">
          <h2 class="text-xl font-semibold">{t("setup_authTitle")}</h2>
          <p class="text-sm text-muted-foreground mt-2">{t("setup_authDesc")}</p>
        </div>

        <div class="grid grid-cols-2 gap-4">
          <!-- OAuth -->
          <button
            class="flex flex-col items-center gap-3 rounded-lg border border-border p-6 text-center transition-colors hover:border-primary/50 hover:bg-accent/50"
            onclick={startOAuthLogin}
          >
            <svg
              class="h-8 w-8 text-primary"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
              ><path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4" /><polyline
                points="10 17 15 12 10 7"
              /><line x1="15" x2="3" y1="12" y2="12" /></svg
            >
            <div>
              <p class="font-medium text-sm">{t("setup_oauthTitle")}</p>
              <p class="text-xs text-muted-foreground mt-1">{t("setup_oauthDesc")}</p>
            </div>
            <span
              class="rounded-full bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary"
              >{t("setup_recommended")}</span
            >
          </button>

          <!-- API Key -->
          <button
            class="flex flex-col items-center gap-3 rounded-lg border border-border p-6 text-center transition-colors hover:border-primary/50 hover:bg-accent/50"
            onclick={() => {
              step = "api_key_setup";
            }}
          >
            <svg
              class="h-8 w-8 text-muted-foreground"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
              ><path
                d="m21 2-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0 3 3L22 7l-3-3m-3.5 3.5L19 4"
              /></svg
            >
            <div>
              <p class="font-medium text-sm">{t("setup_apiKeyTitle")}</p>
              <p class="text-xs text-muted-foreground mt-1">{t("setup_apiKeyDesc")}</p>
            </div>
          </button>
        </div>
      </div>
    {:else if step === "oauth_login"}
      <!-- OAuth login in progress -->
      <div class="flex flex-col items-center gap-4 py-8">
        {#if oauthLoading}
          <div
            class="h-8 w-8 border-2 border-primary/30 border-t-primary rounded-full animate-spin"
          ></div>
          <p class="text-sm font-medium">{t("setup_openingBrowser")}</p>
          <p class="text-xs text-muted-foreground text-center">{t("setup_completeBrowser")}</p>
        {/if}

        {#if error}
          <div class="rounded-lg border border-red-500/30 bg-red-500/5 p-3 w-full max-w-sm">
            <p class="text-sm text-red-500">{error}</p>
          </div>
        {/if}

        <button
          class="rounded-md border border-border px-4 py-2 text-xs hover:bg-accent transition-colors mt-4"
          onclick={() => {
            step = "auth_choice";
            error = "";
          }}>{t("setup_back")}</button
        >
      </div>
    {:else if step === "api_key_setup"}
      <!-- API key setup with platform selection -->
      <div class="flex flex-col gap-5">
        <div class="flex items-center gap-2">
          <button
            class="rounded-md p-1 hover:bg-accent transition-colors"
            onclick={() => {
              step = "auth_choice";
              selectedPlatform = null;
              error = "";
            }}
          >
            <svg
              class="h-4 w-4"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"><path d="m15 18-6-6 6-6" /></svg
            >
          </button>
          <h2 class="text-lg font-semibold">{t("setup_selectPlatform")}</h2>
        </div>

        {#if !selectedPlatform}
          <!-- Platform grid -->
          <div class="flex flex-col gap-4 max-h-[60vh] overflow-y-auto pr-1">
            {#each PRESET_CATEGORIES as category}
              {@const presets = PLATFORM_PRESETS.filter((p) => p.category === category.id)}
              {#if presets.length > 0}
                <div>
                  <p
                    class="text-[11px] font-medium text-muted-foreground uppercase tracking-wider mb-2"
                  >
                    {category.label}
                  </p>
                  <div class="grid grid-cols-3 gap-2">
                    {#each presets as preset}
                      <button
                        class="flex flex-col gap-0.5 rounded-lg border border-border p-3 text-left transition-colors hover:border-primary/50 hover:bg-accent/50"
                        onclick={() => selectPlatform(preset)}
                      >
                        <span class="text-sm font-medium truncate">{preset.name}</span>
                        <span class="text-[10px] text-muted-foreground truncate"
                          >{preset.description}</span
                        >
                      </button>
                    {/each}
                  </div>
                </div>
              {/if}
            {/each}
          </div>
        {:else}
          <!-- Platform config form -->
          <div class="flex flex-col gap-4">
            <div
              class="flex items-center gap-2 rounded-lg border border-primary/30 bg-primary/5 p-3"
            >
              <span class="font-medium text-sm">{selectedPlatform.name}</span>
              <span class="text-xs text-muted-foreground">{selectedPlatform.description}</span>
              <button
                class="ml-auto text-xs text-muted-foreground hover:text-foreground transition-colors"
                onclick={() => {
                  selectedPlatform = null;
                }}>{t("setup_change")}</button
              >
            </div>

            <!-- Custom: extra Base URL input -->
            {#if selectedPlatform.id === "custom"}
              <div class="flex flex-col gap-1.5">
                <label class="text-xs font-medium text-muted-foreground">{t("setup_baseUrl")}</label
                >
                <input
                  type="text"
                  bind:value={customBaseUrl}
                  placeholder="https://api.example.com"
                  class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm focus:outline-none focus:border-ring"
                />
              </div>
            {/if}

            <!-- API Key input -->
            <div class="flex flex-col gap-1.5">
              <label class="text-xs font-medium text-muted-foreground"
                >{t("setup_apiKeyLabel")}</label
              >
              <div class="relative">
                <input
                  type={showKey ? "text" : "password"}
                  bind:value={apiKey}
                  placeholder={selectedPlatform.key_placeholder}
                  class="w-full rounded-md border border-border bg-background px-3 py-2 pr-16 text-sm font-mono focus:outline-none focus:border-ring"
                />
                <button
                  class="absolute right-2 top-1/2 -translate-y-1/2 rounded px-2 py-0.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
                  onclick={() => {
                    showKey = !showKey;
                  }}>{showKey ? t("setup_hide") : t("setup_show")}</button
                >
              </div>
              {#if selectedPlatform.id === "ollama"}
                <p class="text-xs text-muted-foreground">{t("setup_noKeyNeeded")}</p>
              {/if}
            </div>

            <!-- Auth type info -->
            <p class="text-xs text-muted-foreground">
              {selectedPlatform.auth_env_var === "ANTHROPIC_API_KEY"
                ? t("setup_authTypeApiKey")
                : t("setup_authTypeBearer")}
            </p>

            {#if error}
              <div class="rounded-lg border border-red-500/30 bg-red-500/5 p-2">
                <p class="text-xs text-red-500">{error}</p>
              </div>
            {/if}

            <button
              class="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50"
              disabled={saving || (selectedPlatform.id !== "ollama" && !apiKey)}
              onclick={saveApiKey}
            >
              {#if saving}
                <span class="flex items-center gap-2 justify-center">
                  <span
                    class="h-3 w-3 border border-primary-foreground/30 border-t-primary-foreground rounded-full animate-spin"
                  ></span>
                  {t("setup_saving")}
                </span>
              {:else}
                {t("setup_saveAndContinue")}
              {/if}
            </button>
          </div>
        {/if}
      </div>
    {:else if step === "done"}
      <!-- Done! -->
      <div class="flex flex-col items-center gap-4 py-16">
        <div class="flex h-16 w-16 items-center justify-center rounded-full bg-green-500/10">
          <svg
            class="h-8 w-8 text-green-500"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"><path d="M20 6 9 17l-5-5" /></svg
          >
        </div>
        <h2 class="text-xl font-semibold">{t("setup_allSet")}</h2>
        <p class="text-sm text-muted-foreground">{t("setup_allSetDesc")}</p>
        <button
          class="rounded-md bg-primary px-6 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors mt-2"
          onclick={finishNow}>{t("setup_start")}</button
        >
      </div>
    {/if}
  </div>
</div>
