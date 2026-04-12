import { invoke } from "@tauri-apps/api/core";

export interface S3Config {
  endpoint: string;
  bucket: string;
  region: string;
  access_key: string;
  secret_key: string;
}

export async function s3Configure(config: S3Config): Promise<void> {
  return invoke("s3_configure", { config });
}

export async function s3GetConfig(): Promise<S3Config | null> {
  return invoke<S3Config | null>("s3_get_config");
}

export async function s3TestConnection(): Promise<string> {
  return invoke<string>("s3_test_connection");
}

export async function s3Upload(key: string, data: Uint8Array): Promise<void> {
  return invoke("s3_upload", { key, data: Array.from(data) });
}

export async function s3Download(key: string): Promise<Uint8Array> {
  const result = await invoke<number[]>("s3_download", { key });
  return new Uint8Array(result);
}

export async function s3List(prefix: string): Promise<string[]> {
  return invoke<string[]>("s3_list", { prefix });
}

export async function s3Delete(key: string): Promise<void> {
  return invoke("s3_delete", { key });
}
