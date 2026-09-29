// `userAgentData` is Chromium-only, so the WebKit webviews fall back to the user-agent string.
const nav =
	typeof navigator === 'undefined'
		? null
		: (navigator as Navigator & { userAgentData?: { platform?: string } });
const hint = nav?.userAgentData?.platform ?? '';
const agent = nav?.userAgent ?? '';

export const IS_WINDOWS = /Win/i.test(hint) || /Windows/i.test(agent);
export const IS_ANDROID = /Android/i.test(hint) || /Android/i.test(agent);
