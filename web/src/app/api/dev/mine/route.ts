import { connection } from "next/server";

import type { MineResponse } from "@/lib/api-types";
import { appConfig } from "@/lib/config";
import { rpcCall } from "@/lib/server/chain";

/**
 * Dev-only convenience: advance Anvil so `submitVDFProof` clears its
 * `requiredDelay` without a ten-block wait. Disabled outside chain 31337 and
 * when `ALLOW_DEV_MINE=false`.
 */
export async function POST(request: Request) {
  await connection();

  if (appConfig.chainId !== 31337) {
    return Response.json(
      { error: "block mining is only available on the local Anvil chain" },
      { status: 400 },
    );
  }
  if (process.env.ALLOW_DEV_MINE === "false") {
    return Response.json({ error: "dev mining is disabled" }, { status: 403 });
  }

  const body = (await request.json().catch(() => null)) as
    | { blocks?: number }
    | null;
  const requested = Number(body?.blocks ?? 12);
  const blocks = Math.min(1_000, Math.max(1, Number.isFinite(requested) ? requested : 12));

  try {
    await rpcCall<string>("anvil_mine", [`0x${blocks.toString(16)}`]);
    const blockNumber = await rpcCall<string>("eth_blockNumber", []);
    const payload: MineResponse = {
      status: "ok",
      mined: blocks,
      blockNumber: Number(BigInt(blockNumber)),
    };
    return Response.json(payload);
  } catch (cause) {
    return Response.json(
      { error: cause instanceof Error ? cause.message : String(cause) },
      { status: 500 },
    );
  }
}
