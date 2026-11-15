/** Turn wallet/RPC/fetch errors into something a demo user can read. */
export function errorMessage(error: unknown): string {
  if (!error) return "Unknown error";
  if (typeof error === "string") return error;

  const candidate = error as {
    shortMessage?: string;
    details?: string;
    message?: string;
    cause?: { shortMessage?: string; message?: string };
  };

  if (candidate.shortMessage) {
    return candidate.details
      ? `${candidate.shortMessage}: ${candidate.details}`
      : candidate.shortMessage;
  }
  if (candidate.cause?.shortMessage) return candidate.cause.shortMessage;
  if (candidate.message) return candidate.message;
  return "Transaction failed";
}
