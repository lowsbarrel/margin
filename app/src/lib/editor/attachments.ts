// Not dot-prefixed: the watcher, sync and the zip export all skip hidden paths.
export const DEFAULT_ATTACHMENT_FOLDER = 'attachments';

export function resolveAttachmentFolder(setting: string | null | undefined): string {
	return setting?.trim() || DEFAULT_ATTACHMENT_FOLDER;
}
