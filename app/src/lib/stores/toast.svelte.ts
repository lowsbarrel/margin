export type ToastType = 'success' | 'error' | 'info';

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

/**
 * Pending auto-dismiss timers, keyed by toast id.
 *
 * Deliberately a plain object and deliberately NOT reactive: these handles are
 * never rendered and never read from a template, so tracking them would only
 * add bookkeeping to every push/dismiss. `toasts` above is the reactive half of
 * this store; this is bookkeeping that happens to be keyed by the same id.
 */
const timers: Record<number, ReturnType<typeof setTimeout> | undefined> = {};

export const toast = {
	get items() {
		return toasts;
	},

	push(message: string, type: ToastType = 'info', duration = 3000, action?: ToastAction) {
		const id = nextId++;
		toasts = [...toasts, { id, message, type, action }];
		if (duration > 0) {
			const handle = setTimeout(() => {
				delete timers[id];
				toasts = toasts.filter((t) => t.id !== id);
			}, duration);
			timers[id] = handle;
		}
	},

	success(message: string) {
		this.push(message, 'success');
	},

	error(message: string) {
		this.push(message, 'error', 5000);
	},

	info(message: string) {
		this.push(message, 'info');
	},

	dismiss(id: number) {
		const handle = timers[id];
		if (handle !== undefined) {
			clearTimeout(handle);
			delete timers[id];
		}
		toasts = toasts.filter((t) => t.id !== id);
	}
};
