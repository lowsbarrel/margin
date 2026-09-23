// ROW_HEIGHT is the h-8 of ROW_BASE below: the painted rows and the virtual-scroll maths must stay in lockstep.
export const ROW_HEIGHT = 32;

export const ROW_BASE =
	'tree-row relative flex h-8 w-full items-center gap-2 overflow-hidden pr-3 text-sm tracking-normal [&_svg]:shrink-0';
export const ROW_BUTTON = `${ROW_BASE} cursor-pointer rounded-sm text-left transition-colors`;
const ROW_QUIET = 'font-normal text-muted-foreground hover:bg-surface-1 hover:text-foreground';
const ROW_SELECTED = 'font-normal bg-surface-2 text-foreground';

export function folderRowClass(selected: boolean, dropTarget: boolean, focused: boolean) {
	const base = dropTarget
		? `${ROW_BUTTON} font-normal bg-brand/24 text-foreground shadow-[inset_0_0_0_1px_var(--color-border-focus)]`
		: `${ROW_BUTTON} ${selected ? ROW_SELECTED : ROW_QUIET}`;
	return focused ? `${base} is-focused` : base;
}

export function fileRowClass(active: boolean, selected: boolean, focused: boolean) {
	const base = active
		? `${ROW_BUTTON} font-medium bg-accent text-foreground [&_svg]:text-accent-foreground before:absolute before:top-1.5 before:bottom-1.5 before:left-0 before:w-0.5 before:rounded-full before:bg-brand before:content-['']`
		: `${ROW_BUTTON} ${selected ? ROW_SELECTED : ROW_QUIET}`;
	return focused ? `${base} is-focused` : base;
}

export const INLINE_INPUT =
	'h-6 min-w-0 flex-1 rounded-sm border-ring bg-background px-2 py-0.5 text-sm text-foreground shadow-[0_0_0_3px_var(--color-brand-16)] outline-none';
