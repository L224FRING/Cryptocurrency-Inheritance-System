"use client";

import { useState } from "react";

import { errorMessage } from "@/lib/errors";
import { postJson } from "@/lib/client-api";
import { Button } from "@/components/ui";

export function DevMineButton({
  blocks = 12,
  onMined,
}: {
  blocks?: number;
  onMined?: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  return (
    <div className="flex flex-col gap-1">
      <Button
        size="sm"
        variant="secondary"
        busy={busy}
        onClick={async () => {
          setBusy(true);
          setError(null);
          try {
            await postJson("/api/dev/mine", { blocks });
            onMined?.();
          } catch (cause) {
            setError(errorMessage(cause));
          } finally {
            setBusy(false);
          }
        }}
      >
        Advance {blocks} blocks
      </Button>
      {error && <span className="text-[11px] text-rose-300">{error}</span>}
    </div>
  );
}
