// The editor pane clips its own overflow, so viewport-positioned table UI hangs off a fixed body-level layer.
const LAYER_CLASS = 'table-ui-layer';

let layer: HTMLElement | null = null;

export function getTableUiLayer(): HTMLElement {
	if (layer?.isConnected) return layer;
	layer = document.createElement('div');
	layer.className = LAYER_CLASS;
	document.body.appendChild(layer);
	return layer;
}

export function releaseTableUiLayer(): void {
	if (layer?.childElementCount === 0) {
		layer.remove();
		layer = null;
	}
}
