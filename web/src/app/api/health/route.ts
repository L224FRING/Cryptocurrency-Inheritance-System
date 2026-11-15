import { connection } from "next/server";

import type { HealthResponse } from "@/lib/api-types";
import { appConfig } from "@/lib/config";
import { serverClient } from "@/lib/server/chain";
import { frostHealth } from "@/lib/server/frost";

export async function GET() {
  // Reading the filesystem and RPC is request-time work; keep it out of the
  // prerendered shell.
  await connection();

  const contracts = {
    vault: appConfig.vault,
    vdf: appConfig.vdf,
    frost: appConfig.frost,
  };
  const frost = frostHealth();

  try {
    const [chainId, blockNumber] = await Promise.all([
      serverClient.getChainId(),
      serverClient.getBlockNumber(),
    ]);
    const payload: HealthResponse = {
      ok: true,
      chainId,
      blockNumber: Number(blockNumber),
      rpcUrl: appConfig.rpcUrl,
      contracts,
      frost,
    };
    return Response.json(payload);
  } catch (cause) {
    const payload: HealthResponse = {
      ok: false,
      chainId: appConfig.chainId,
      rpcUrl: appConfig.rpcUrl,
      contracts,
      frost,
      error: cause instanceof Error ? cause.message : String(cause),
    };
    return Response.json(payload);
  }
}
