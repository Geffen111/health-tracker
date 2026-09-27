<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { showToast } from '$lib/stores/toast.svelte';

  // Shows one exposure attachment in a frame a little smaller than the window. Formats
  // the webview can't draw (HEIC from a phone) fall back to "open in the default app".
  let { attachmentId, onclose }: { attachmentId: number; onclose: () => void } = $props();

  let fileName = $state('');
  let src = $state('');
  let failed = $state(false);
  let loading = $state(true);

  $effect(() => {
    const id = attachmentId;
    loading = true;
    failed = false;
    src = '';
    invoke<{ file_name: string; mime_type: string; data_base64: string }>('get_exposure_attachment', { id })
      .then((a) => {
        if (id !== attachmentId) return;
        fileName = a.file_name;
        src = `data:${a.mime_type};base64,${a.data_base64}`;
      })
      .catch((e) => { failed = true; showToast(String(e), 'error'); })
      .finally(() => { loading = false; });
  });

  async function openExternally() {
    try {
      await invoke('open_exposure_attachment', { id: attachmentId });
    } catch (e) {
      showToast(`Couldn't open it: ${e}`, 'error');
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose();
  }
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="overlay" role="presentation" onclick={onclose}>
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_interactive_supports_focus -->
  <div class="frame" role="dialog" aria-modal="true" aria-label={fileName || 'Attachment'} tabindex="-1" onclick={(e) => e.stopPropagation()}>
    <div class="bar">
      <span class="name">{fileName}</span>
      <button class="bar-btn" onclick={openExternally} title="Open in the default app">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5"/></svg>
        Open
      </button>
      <button class="bar-btn close" onclick={onclose} aria-label="Close">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18"/></svg>
      </button>
    </div>
    <div class="stage">
      {#if loading}
        <p class="msg">Loading…</p>
      {:else if failed || !src}
        <p class="msg">This image can't be shown here — use <strong>Open</strong> to view it in your default app.</p>
      {:else}
        <img {src} alt={fileName} onerror={() => (failed = true)} />
      {/if}
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed; inset: 0; z-index: 2400;
    background: rgba(20, 29, 27, 0.6);
    display: flex; align-items: center; justify-content: center;
    padding: 36px;
  }
  .frame {
    max-width: calc(100vw - 72px); max-height: calc(100vh - 72px);
    min-width: 320px;
    display: flex; flex-direction: column;
    background: var(--card); border: 1px solid var(--border); border-radius: 16px;
    box-shadow: var(--shadow-lg); overflow: hidden;
  }
  .bar { display: flex; align-items: center; gap: 8px; padding: 10px 12px 10px 16px; border-bottom: 1px solid var(--border); }
  .name { flex: 1; min-width: 0; font-size: 13px; font-weight: 600; color: var(--tp); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .bar-btn { display: inline-flex; align-items: center; gap: 6px; height: 30px; padding: 0 11px; border-radius: 9px; border: 1px solid var(--border); background: var(--card); color: var(--ts); font-size: 12.5px; font-weight: 700; cursor: pointer; }
  .bar-btn:hover { background: var(--inset); }
  .bar-btn.close { width: 30px; padding: 0; justify-content: center; }
  .stage { flex: 1; min-height: 0; display: flex; align-items: center; justify-content: center; background: var(--inset); }
  img { display: block; max-width: calc(100vw - 74px); max-height: calc(100vh - 124px); object-fit: contain; }
  .msg { padding: 48px 32px; font-size: 13px; color: var(--ts); text-align: center; max-width: 360px; }
</style>
