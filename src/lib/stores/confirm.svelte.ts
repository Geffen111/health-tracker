// Global confirm-dialog store (runes). A single <ConfirmDialog/> instance lives
// in the root layout; any page calls `await confirmAction(...)` to get a themed
// modal in place of the browser's native confirm(), which doesn't match the app.

export interface ConfirmOptions {
  title?: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
}

interface ConfirmState extends ConfirmOptions {
  open: boolean;
  resolve: ((ok: boolean) => void) | null;
}

export const confirmState = $state<ConfirmState>({
  open: false,
  message: '',
  resolve: null,
});

export function confirmAction(opts: ConfirmOptions | string): Promise<boolean> {
  const o: ConfirmOptions = typeof opts === 'string' ? { message: opts } : opts;
  return new Promise((resolve) => {
    confirmState.open = true;
    confirmState.title = o.title ?? 'Are you sure?';
    confirmState.message = o.message;
    confirmState.confirmLabel = o.confirmLabel ?? 'Delete';
    confirmState.cancelLabel = o.cancelLabel ?? 'Cancel';
    confirmState.danger = o.danger ?? true;
    confirmState.resolve = resolve;
  });
}

export function resolveConfirm(ok: boolean) {
  confirmState.resolve?.(ok);
  confirmState.open = false;
  confirmState.resolve = null;
}
