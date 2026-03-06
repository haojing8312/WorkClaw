import { invoke } from "@tauri-apps/api/core";

export async function openExternalUrl(url: string): Promise<void> {
  const trimmed = url.trim();
  if (!trimmed) {
    return;
  }
  await invoke("open_external_url", { url: trimmed });
}
