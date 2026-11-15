"use client";

import { useState } from "react";
import {
  usePublicClient,
  useSendTransaction,
  useWriteContract,
} from "wagmi";
import type { Hash } from "viem";

import { errorMessage } from "@/lib/errors";
import { shorten } from "@/lib/format";

/**
 * Small wrapper around wagmi writes that waits for a receipt and exposes one
 * shared pending/error/status slot. Components call `run(label, action)` and
 * can sequence several transactions inside one action.
 */
export function useTx() {
  const publicClient = usePublicClient();
  const { mutateAsync: writeContractAsync } = useWriteContract();
  const { mutateAsync: sendTransactionAsync } = useSendTransaction();

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [hash, setHash] = useState<Hash | null>(null);

  type WriteRequest = Parameters<typeof writeContractAsync>[0];
  type SendRequest = Parameters<typeof sendTransactionAsync>[0];

  async function write(request: WriteRequest, label = "Transaction") {
    const txHash = await writeContractAsync(request);
    setHash(txHash);
    setNote(`${label} submitted (${shorten(txHash)}) — waiting…`);
    if (!publicClient) throw new Error("No RPC client available");
    const receipt = await publicClient.waitForTransactionReceipt({ hash: txHash });
    if (receipt.status !== "success") {
      throw new Error(`${label} reverted in block ${receipt.blockNumber}`);
    }
    setNote(`${label} confirmed in block ${receipt.blockNumber}`);
    return receipt;
  }

  async function send(request: SendRequest, label = "Transfer") {
    const txHash = await sendTransactionAsync(request);
    setHash(txHash);
    setNote(`${label} submitted (${shorten(txHash)}) — waiting…`);
    if (!publicClient) throw new Error("No RPC client available");
    const receipt = await publicClient.waitForTransactionReceipt({ hash: txHash });
    if (receipt.status !== "success") {
      throw new Error(`${label} reverted in block ${receipt.blockNumber}`);
    }
    setNote(`${label} confirmed in block ${receipt.blockNumber}`);
    return receipt;
  }

  /** Runs an action with shared busy/error state. Errors are surfaced, not thrown. */
  async function run(label: string, action: () => Promise<unknown>) {
    setBusy(true);
    setError(null);
    setNote(`${label}…`);
    try {
      await action();
      return true;
    } catch (cause) {
      setError(errorMessage(cause));
      setNote(null);
      return false;
    } finally {
      setBusy(false);
    }
  }

  function reset() {
    setError(null);
    setNote(null);
    setHash(null);
  }

  return { busy, error, note, hash, write, send, run, reset };
}
