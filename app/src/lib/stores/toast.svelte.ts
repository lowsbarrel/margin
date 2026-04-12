export type ToastType = "success" | "error" | "info";

export interface ToastAction {
  label: string;
  onClick: () => void;
}

interface Toast {
  id: number;
  message: string;
  type: ToastType;
  action?: ToastAction;
}

let nextId = 0;
let toasts = $state<Toast[]>([]);

export const toast = {
  get items() {
    return toasts;
  },

  push(message: string, type: ToastType = "info", duration = 3000, action?: ToastAction) {
    const id = nextId++;
    toasts = [...toasts, { id, message, type, action }];
    if (duration > 0) {
      setTimeout(() => {
        toasts = toasts.filter((t) => t.id !== id);
      }, duration);
    }
  },

  success(message: string) {
    this.push(message, "success");
  },

  error(message: string) {
    this.push(message, "error", 5000);
  },

  info(message: string) {
    this.push(message, "info");
  },

  dismiss(id: number) {
    toasts = toasts.filter((t) => t.id !== id);
  },
};
