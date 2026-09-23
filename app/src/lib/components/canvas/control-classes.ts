// app.css styles every bare <button> in its base layer, so these restate padding and radius.

const TOOL_BUTTON =
	'flex size-7.5 min-h-7.5 min-w-7.5 shrink-0 items-center justify-center rounded-xs bg-transparent p-0 [transition:background_var(--transition-fast),color_var(--transition-fast)]';

const SWATCH =
	'size-4.5 min-h-4.5 min-w-4.5 shrink-0 box-content rounded-full border-2 p-0 shadow-[inset_0_0_0_1px_var(--color-border-strong)] [transition:border-color_var(--transition-fast),scale_var(--transition-fast)]';

const SIZE_BUTTON =
	'size-7 min-h-7 min-w-7 rounded-xs border bg-transparent p-0 text-xs [transition:background_var(--transition-fast),color_var(--transition-fast),border-color_var(--transition-fast)]';

export const MENU_ITEM =
	'block w-full rounded-xs bg-transparent px-2.5 py-1.5 text-left text-sm [transition:background_var(--transition-fast),color_var(--transition-fast)]';

export const toolButtonClass = (active: boolean) =>
	`${TOOL_BUTTON} ${
		active
			? 'bg-surface-2 text-foreground'
			: 'text-subtle-foreground hover:bg-surface-3 hover:text-foreground'
	}`;

export const swatchClass = (active: boolean) =>
	`${SWATCH} ${active ? 'border-foreground scale-115' : 'border-transparent hover:scale-120'}`;

export const menuItemClass = (active: boolean) =>
	`${MENU_ITEM} hover:bg-surface-3 hover:text-foreground ${
		active ? 'text-foreground' : 'text-muted-foreground'
	}`;

export const sizeButtonClass = (active: boolean) =>
	`${SIZE_BUTTON} ${
		active
			? 'border-foreground bg-surface-2 text-foreground'
			: 'border-border text-muted-foreground hover:bg-surface-3'
	}`;
