// `navigator.userAgentData` is Chromium-only (WebView2); the user-agent string is
// the fallback for the WebKit webviews, and `navigator.platform` is deprecated.
const nav =
	typeof navigator === 'undefined'
		? null
		: (navigator as Navigator & { userAgentData?: { platform?: string } });
const hint = nav?.userAgentData?.platform ?? '';
const agent = nav?.userAgent ?? '';

export const IS_WINDOWS = /Win/i.test(hint) || /Windows/i.test(agent);
export const IS_ANDROID = /Android/i.test(hint) || /Android/i.test(agent);
