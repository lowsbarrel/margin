// Floating table UI is positioned in viewport coordinates, so it hangs off a
// fixed layer on `body`: the editor pane clips its own overflow and would cut
// the handles near its edges.

const LAYER_CLASS = 'table-ui-layer';

let layer: HTMLElement | null = null;

export function getTableUiLayer(): HTMLElement {
	if (layer?.isConnected) return layer;
	layer = document.createElement('div');
	layer.className = LAYER_CLASS;
	document.body.appendChild(layer);
	return layer;
}

/** Drop the layer once nothing is left inside it. */
export function releaseTableUiLayer(): void {
	if (layer?.childElementCount === 0) {
		layer.remove();
		layer = null;
	}
}
