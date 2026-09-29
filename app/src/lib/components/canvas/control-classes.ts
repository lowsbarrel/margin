// app.css styles every bare <button> in its base layer, so these restate padding and radius.

const ICON_BUTTON =
	'flex size-8 min-h-8 min-w-8 shrink-0 items-center justify-center rounded-xs bg-transparent p-0 [transition:background_var(--transition-fast),color_var(--transition-fast)]';

const SWATCH =
	'size-6 min-h-6 min-w-6 shrink-0 box-content rounded-full border-2 p-0 shadow-[inset_0_0_0_1px_var(--color-border-strong)] [transition:border-color_var(--transition-fast),scale_var(--transition-fast)]';

export const MENU_ITEM =
	'block w-full rounded-xs bg-transparent px-2.5 py-1.5 text-left text-sm [transition:background_var(--transition-fast),color_var(--transition-fast)]';

export const toolButtonClass = (active: boolean) =>
	`${ICON_BUTTON} ${
		active
			? 'bg-surface-2 text-foreground'
			: 'text-subtle-foreground hover:bg-surface-3 hover:text-foreground'
	}`;

export const colorButtonClass = (active: boolean) =>
	`${ICON_BUTTON} ${active ? 'bg-surface-2' : 'hover:bg-surface-3'}`;

export const swatchClass = (active: boolean) =>
	`${SWATCH} ${active ? 'border-foreground scale-115' : 'border-transparent hover:scale-120'}`;
