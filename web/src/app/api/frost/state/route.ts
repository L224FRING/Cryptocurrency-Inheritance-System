import { connection } from "next/server";

import type { FrostStateResponse } from "@/lib/api-types";
import { readState, shareIndices } from "@/lib/server/frost";

export async function GET() {
  await connection();

  const state = readState();
  if (!state) {
    const payload: FrostStateResponse = {
      configured: false,
      shares: shareIndices(),
    };
    return Response.json(payload);
  }

  const payload: FrostStateResponse = {
    configured: true,
    trustees: state.trustees,
    threshold: state.threshold,
    groupKey: state.groupKey,
    session: state.session,
    createdAt: state.createdAt,
    shares: state.shares,
  };
  return Response.json(payload);
}
