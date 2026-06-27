// Tiny global toast store (runes). A single <Toast/> instance lives in the root
// layout; any page calls showToast(...) to surface transient feedback.

export type ToastKind = 'success' | 'error' | 'info';

export interface Toast {
  id: number;
  message: string;
  kind: ToastKind;
}

let nextId = 0;

export const toasts = $state<{ items: Toast[] }>({ items: [] });

export function showToast(message: string, kind: ToastKind = 'success', timeout = 3200) {
  const id = nextId++;
  toasts.items.push({ id, message, kind });
  if (timeout > 0) {
    setTimeout(() => dismissToast(id), timeout);
  }
}

export function dismissToast(id: number) {
  toasts.items = toasts.items.filter((t) => t.id !== id);
}
