import type { DkgRequest, DkgResponse } from "@/lib/api-types";
import { runDkg } from "@/lib/server/frost";

export async function POST(request: Request) {
  const body = (await request.json().catch(() => null)) as
    | Partial<DkgRequest>
    | null;

  const trustees = Number(body?.trustees);
  const threshold = Number(body?.threshold);

  if (!Number.isInteger(trustees) || trustees < 2 || trustees > 10) {
    return Response.json(
      { error: "trustees must be an integer between 2 and 10" },
      { status: 400 },
    );
  }
  if (!Number.isInteger(threshold) || threshold < 2 || threshold > trustees) {
    return Response.json(
      { error: "threshold must be an integer between 2 and trustees" },
      { status: 400 },
    );
  }

  try {
    const state = await runDkg(trustees, threshold);
    const payload: DkgResponse = {
      status: "ok",
      trustees: state.trustees,
      threshold: state.threshold,
      groupKey: state.groupKey,
      session: state.session,
      shares: state.shares,
    };
    return Response.json(payload);
  } catch (cause) {
    return Response.json(
      { error: cause instanceof Error ? cause.message : String(cause) },
      { status: 500 },
    );
  }
}
