export const toBytes = (u: Uint8Array): number[] => Array.from(u);

export const fromBytes = (a: number[] | ArrayBuffer): Uint8Array =>
	a instanceof ArrayBuffer ? new Uint8Array(a) : Uint8Array.from(a);
