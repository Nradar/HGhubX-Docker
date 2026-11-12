import { isWeb, shellCall } from "./transport";
import type {
  GateState,
  UpdateInfo,
} from "./types";

/** 壳层命令门面：只放非业务命令，业务数据统一走 data/client.ts。 */

/** 写一行到应用数据目录 debug.log（用户反馈「打不开」时要的就是它）。 */
export const logLine = (msg: string) => shellCall("debug_log", { msg }).catch(() => {});

export const gateState = () => shellCall<GateState>("gate_state");
export const gateOpen = (address: string) => shellCall<GateState>("gate_open", { address });
export const gateReset = () => shellCall<GateState>("gate_reset");

export const securitySet = (strict: boolean) => shellCall<boolean>("security_set", { strict });

export const clearCache = () => shellCall<void>("cache_clear");
export const netDiag = () => shellCall<Record<string, unknown>>("net_diag");
/** 系统方向请求；页面不直接 shellCall，统一从门面走。 */
export const setOrientation = (landscape: boolean) => isWeb ? Promise.resolve() : shellCall<void>("set_orientation", { landscape });

/* 更新与应用控制 */
export const checkUpdate = () => shellCall<UpdateInfo>("check_update");
export async function openUrl(raw: string): Promise<void> {
  if (!isWeb) return shellCall<void>("open_url", { url: raw });
  const url = new URL(raw);
  if (url.protocol !== "https:" && url.protocol !== "http:") throw new Error("链接协议不受支持");
  window.open(url.href, "_blank", "noopener,noreferrer");
}

/** iOS 文本交互开关：长按快进时避免 WKWebView 放大镜。 */
export const setTextInteraction = (enabled: boolean) =>
  isWeb ? Promise.resolve() : shellCall<void>("set_text_interaction", { enabled });

/** 安卓硬件返回键走到桌面后退出应用。 */
export const exitApp = () => isWeb ? Promise.resolve() : shellCall<void>("plugin:app|exit", { code: 0 });
