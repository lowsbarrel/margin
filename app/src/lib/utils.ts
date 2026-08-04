import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

/**
 * Merge Tailwind class lists, with later classes winning conflicts.
 *
 * `clsx` flattens conditionals/arrays into a string; `twMerge` then resolves
 * collisions so a caller-supplied `px-4` replaces a component's built-in `px-2`
 * instead of both landing in the class list and letting source order decide.
 * This is the convention every shadcn-svelte component expects.
 */
export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & {
	ref?: U | null;
};

/**
 * Type helpers that generated shadcn-svelte components import from `$lib/utils`.
 *
 * They must live here rather than anywhere else: every future
 * `shadcn-svelte add` emits components with `import type { WithoutChild } from
 * '$lib/utils'` hardcoded, so keeping them at this path means new components
 * drop in without a fixup pass.
 *
 * `child` and `children` are Snippet props on the underlying bits-ui
 * primitives. A wrapper that renders its own markup strips them from the props
 * it forwards, so a caller cannot accidentally replace the wrapper's internals.
 */

/** Drop the `child` snippet prop, if present. */
export type WithoutChild<T> = T extends { child?: unknown } ? Omit<T, 'child'> : T;

/** Drop the `children` snippet prop, if present. */
export type WithoutChildren<T> = T extends { children?: unknown } ? Omit<T, 'children'> : T;

/** Drop both `child` and `children`. */
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>;
