import { invoke } from "@tauri-apps/api/core";
import { commands } from "$lib/bindings";
import { fromBytes } from "$lib/ipc";

export type { S3Config } from "$lib/bindings";
import type { S3Config } from "$lib/bindings";

export async function s3Configure(config: S3Config): Promise<void> {
  const r = await commands.s3Configure(config);
  if (r.status === "error") throw r.error;
}

export async function s3GetConfig(): Promise<S3Config | null> {
  const r = await commands.s3GetConfig();
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function s3TestConnection(): Promise<string> {
  const r = await commands.s3TestConnection();
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function s3Upload(key: string, data: Uint8Array): Promise<void> {
  return invoke<void>("s3_upload", data, {
    headers: { "x-key": key },
  });
}

export async function s3Download(key: string): Promise<Uint8Array> {
  const buffer = await invoke<ArrayBuffer>("s3_download", { key });
  return fromBytes(buffer);
}

export async function s3List(prefix: string): Promise<string[]> {
  const r = await commands.s3List(prefix);
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function s3Delete(key: string): Promise<void> {
  const r = await commands.s3Delete(key);
  if (r.status === "error") throw r.error;
}
