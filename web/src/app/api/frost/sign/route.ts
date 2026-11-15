import type { SignRequest, SignResponse } from "@/lib/api-types";
import { runSign } from "@/lib/server/frost";

export async function POST(request: Request) {
  const body = (await request.json().catch(() => null)) as
    | Partial<SignRequest>
    | null;

  const releaseHash = body?.releaseHash;
  if (typeof releaseHash !== "string") {
    return Response.json(
      { error: "releaseHash is required" },
      { status: 400 },
    );
  }

  let participants: number[] | undefined;
  if (body?.participants !== undefined) {
    if (
      !Array.isArray(body.participants) ||
      body.participants.some((value) => !Number.isInteger(Number(value)))
    ) {
      return Response.json(
        { error: "participants must be an array of trustee indices" },
        { status: 400 },
      );
    }
    participants = body.participants.map(Number);
  }

  try {
    const result = await runSign(releaseHash, participants);
    const payload: SignResponse = {
      status: "ok",
      signature: result.signature,
      groupKey: result.groupKey,
      participants: result.participants,
      session: result.session,
    };
    return Response.json(payload);
  } catch (cause) {
    return Response.json(
      { error: cause instanceof Error ? cause.message : String(cause) },
      { status: 500 },
    );
  }
}
