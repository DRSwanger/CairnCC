<script lang="ts">
  import Modal from "./Modal.svelte";
  import PermissionsEditor from "./PermissionsEditor.svelte";
  import { t } from "$lib/i18n/index.svelte";

  let {
    open = $bindable(false),
    cwd = "",
  }: {
    open: boolean;
    cwd?: string;
  } = $props();
</script>

<Modal bind:open title={t("permissions_title")}>
  {#if open}
    <PermissionsEditor {cwd} />
  {/if}
</Modal>
