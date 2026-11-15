import type { VdfProveRequest, VdfProveResponse } from "@/lib/api-types";
import { runVdf } from "@/lib/server/frost";

export async function POST(request: Request) {
  const body = (await request.json().catch(() => null)) as
    | Partial<VdfProveRequest>
    | null;

  const challenge = body?.challenge;
  const n = body?.n;
  const t = Number(body?.t);

  if (typeof challenge !== "string") {
    return Response.json({ error: "challenge is required" }, { status: 400 });
  }
  if (typeof n !== "string") {
    return Response.json({ error: "n is required" }, { status: 400 });
  }
  if (!Number.isInteger(t) || t < 1 || t > 1_000_000) {
    return Response.json(
      { error: "t must be an integer between 1 and 1,000,000" },
      { status: 400 },
    );
  }

  try {
    // Validate before spawning the prover; BigInt throws on malformed input.
    BigInt(challenge);
    BigInt(n);
    BigInt(t);
  } catch {
    return Response.json(
      { error: "challenge, n and t must be numeric" },
      { status: 400 },
    );
  }

  try {
    const result = await runVdf(challenge, t, n);
    const payload: VdfProveResponse = {
      status: "ok",
      y: result.y,
      proofPoints: result.proofPoints,
      x: result.x,
      t: result.t,
      n: result.n,
    };
    return Response.json(payload);
  } catch (cause) {
    return Response.json(
      { error: cause instanceof Error ? cause.message : String(cause) },
      { status: 500 },
    );
  }
}
