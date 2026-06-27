<script lang="ts">
  import { toasts, dismissToast } from '$lib/stores/toast.svelte';
</script>

<div class="toast-stack">
  {#each toasts.items as t (t.id)}
    <button class="toast {t.kind}" onclick={() => dismissToast(t.id)} title="Dismiss">
      <span class="toast-dot"></span>
      <span class="toast-msg">{t.message}</span>
    </button>
  {/each}
</div>

<style>
  .toast-stack {
    position: fixed;
    top: 20px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 2000;
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: center;
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    display: inline-flex;
    align-items: center;
    gap: 10px;
    max-width: 440px;
    padding: 11px 16px;
    border-radius: 999px;
    border: 1px solid var(--border);
    background: var(--card);
    color: var(--tp);
    font-family: inherit;
    font-size: 13px;
    font-weight: 600;
    text-align: left;
    cursor: pointer;
    box-shadow: var(--shadow-lg);
    animation: toast-in 0.18s ease-out;
  }
  .toast-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--accent);
  }
  .toast.success .toast-dot { background: var(--accent); }
  .toast.error .toast-dot { background: var(--red); }
  .toast.info .toast-dot { background: var(--peri); }
  .toast.error { color: var(--red-fg); }
  .toast-msg { line-height: 1.35; }
  @keyframes toast-in {
    from { opacity: 0; transform: translateY(-6px); }
    to { opacity: 1; transform: translateY(0); }
  }
</style>
