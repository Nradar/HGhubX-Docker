import { invoke } from "@tauri-apps/api/core";

export const isWeb = import.meta.env.MODE === "web";

export async function shellCall<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  if (!isWeb) return invoke<T>(command, args);

  const response = await fetch(`/api/${command}`, {
    method: "POST",
    credentials: "same-origin",
    headers: { "Content-Type": "application/json", "X-HGhubX-Web": "1" },
    body: JSON.stringify(args),
  }).catch(() => { throw "网络连接失败，请检查 Web 服务"; });
  if (response.status === 401) throw "Web 登录已失效，请刷新页面";
  const result = await response.json().catch(() => { throw `HTTP ${response.status}`; });
  if (!response.ok) throw result?.error || `HTTP ${response.status}`;
  return result as T;
}
