import { createPublicClient, defineChain, http } from "viem";

import { appConfig } from "@/lib/config";

/** Server-side read-only client. Mirrors the chain config used by wagmi. */
export const serverChain = defineChain({
  id: appConfig.chainId,
  name: appConfig.chainId === 31337 ? "Anvil Local" : `Chain ${appConfig.chainId}`,
  nativeCurrency: { name: "Ether", symbol: "ETH", decimals: 18 },
  rpcUrls: {
    default: { http: [appConfig.rpcUrl] },
  },
  testnet: true,
});

export const serverClient = createPublicClient({
  chain: serverChain,
  transport: http(appConfig.rpcUrl, { timeout: 3_000, retryCount: 0 }),
});

/** Raw JSON-RPC helper for Anvil-only methods (`anvil_mine`). */
export async function rpcCall<T>(
  method: string,
  params: unknown[] = [],
): Promise<T> {
  const response = await fetch(appConfig.rpcUrl, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
    cache: "no-store",
  });
  if (!response.ok) {
    throw new Error(`RPC ${method} failed with HTTP ${response.status}`);
  }
  const payload = (await response.json()) as {
    result?: T;
    error?: { message?: string };
  };
  if (payload.error) {
    throw new Error(payload.error.message ?? `RPC ${method} failed`);
  }
  return payload.result as T;
}
