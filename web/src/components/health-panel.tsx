"use client";

import { useQuery } from "@tanstack/react-query";

import type { HealthResponse } from "@/lib/api-types";
import { getJson } from "@/lib/client-api";
import { appConfig, isConfigured } from "@/lib/config";
import { shorten } from "@/lib/format";
import { Alert, Badge, Card, KeyValue } from "@/components/ui";

export function HealthPanel() {
  const health = useQuery<HealthResponse>({
    queryKey: ["health"],
    queryFn: () => getJson<HealthResponse>("/api/health"),
    refetchInterval: 10_000,
  });

  const data = health.data;

  return (
    <Card
      title="System status"
      description="Live checks against the local chain and the FROST helper binaries."
      actions={
        data?.ok ? (
          <Badge tone="success">RPC online</Badge>
        ) : data ? (
          <Badge tone="danger">RPC offline</Badge>
        ) : (
          <Badge tone="neutral">Checking…</Badge>
        )
      }
    >
      {!isConfigured && (
        <div className="mb-4">
          <Alert tone="warning" title="Contract addresses are not configured">
            Run <code className="font-mono">web/scripts/deploy-local.sh</code>{" "}
            (with Anvil running) to deploy the contracts and write{" "}
            <code className="font-mono">web/.env.local</code>, then restart{" "}
            <code className="font-mono">pnpm dev</code>.
          </Alert>
        </div>
      )}

      {health.isError && (
        <Alert tone="danger" title="Health request failed">
          {(health.error as Error).message}
        </Alert>
      )}

      {data && (
        <div className="space-y-3">
          {!data.ok && (
            <Alert tone="danger" title="Cannot reach the RPC">
              {data.error ?? "Unknown RPC error"} — start Anvil with{" "}
              <code className="font-mono">anvil</code>.
            </Alert>
          )}

          <dl>
            <KeyValue label="RPC">{data.rpcUrl}</KeyValue>
            <KeyValue label="Chain id">{data.chainId}</KeyValue>
            <KeyValue label="Block">{data.blockNumber ?? "—"}</KeyValue>
            <KeyValue label="Vault">{shorten(data.contracts.vault, 10, 8)}</KeyValue>
            <KeyValue label="VDF verifier">
              {shorten(data.contracts.vdf, 10, 8)}
            </KeyValue>
            <KeyValue label="FROST verifier">
              {shorten(data.contracts.frost, 10, 8)}
            </KeyValue>
            <KeyValue label="frost-service">
              {data.frost.serviceBinExists ? (
                <Badge tone="success">found</Badge>
              ) : (
                <Badge tone="danger">missing</Badge>
              )}
            </KeyValue>
            <KeyValue label="frost-relay">
              {data.frost.relayBinExists ? (
                <Badge tone="success">found</Badge>
              ) : (
                <Badge tone="danger">missing</Badge>
              )}
            </KeyValue>
            <KeyValue label="FROST shares">
              {data.frost.hasShares ? (
                <Badge tone="success">present</Badge>
              ) : (
                <Badge tone="neutral">none yet</Badge>
              )}
            </KeyValue>
          </dl>

          {(!data.frost.serviceBinExists || !data.frost.relayBinExists) && (
            <Alert tone="warning" title="Build the FROST binaries">
              <code className="font-mono">
                cargo build --manifest-path rust/frost-service/Cargo.toml
              </code>
            </Alert>
          )}

          <p className="text-xs text-slate-500">
            All activity is local-only. Chain id {appConfig.chainId}; contracts
            are re-deployed each time you run the local deploy script.
          </p>
        </div>
      )}
    </Card>
  );
}
