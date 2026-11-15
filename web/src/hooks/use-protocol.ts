"use client";

import { useQuery } from "@tanstack/react-query";
import {
  useBalance,
  useBlockNumber,
  useReadContract,
} from "wagmi";

import type { FrostStateResponse } from "@/lib/api-types";
import { appConfig, isConfigured } from "@/lib/config";
import { frostAbi, vdfAbi, vaultAbi } from "@/lib/contracts";

const LIVE = { enabled: isConfigured, refetchInterval: 4_000 } as const;

/** Read every contract field the owner and trustee dashboards need. */
export function useProtocolState() {
  const block = useBlockNumber({ watch: true });
  const vaultBalance = useBalance({
    address: appConfig.vault,
    query: LIVE,
  });

  const vaultOwner = useReadContract({
    address: appConfig.vault,
    abi: vaultAbi,
    functionName: "owner",
    query: LIVE,
  });
  const beneficiary = useReadContract({
    address: appConfig.vault,
    abi: vaultAbi,
    functionName: "beneficiary",
    query: LIVE,
  });
  const beneficiarySet = useReadContract({
    address: appConfig.vault,
    abi: vaultAbi,
    functionName: "beneficiarySet",
    query: LIVE,
  });
  const released = useReadContract({
    address: appConfig.vault,
    abi: vaultAbi,
    functionName: "released",
    query: LIVE,
  });
  const vaultChallenge = useReadContract({
    address: appConfig.vault,
    abi: vaultAbi,
    functionName: "currentChallenge",
    query: LIVE,
  });
  const vaultLastCheckInBlock = useReadContract({
    address: appConfig.vault,
    abi: vaultAbi,
    functionName: "lastCheckInBlock",
    query: LIVE,
  });
  const vaultLastCheckInTime = useReadContract({
    address: appConfig.vault,
    abi: vaultAbi,
    functionName: "lastCheckInTime",
    query: LIVE,
  });

  const vdfN = useReadContract({
    address: appConfig.vdf,
    abi: vdfAbi,
    functionName: "N",
    query: LIVE,
  });
  const vdfT = useReadContract({
    address: appConfig.vdf,
    abi: vdfAbi,
    functionName: "T",
    query: LIVE,
  });
  const vdfChallenge = useReadContract({
    address: appConfig.vdf,
    abi: vdfAbi,
    functionName: "currentChallenge",
    query: LIVE,
  });
  const vdfLastCheckIn = useReadContract({
    address: appConfig.vdf,
    abi: vdfAbi,
    functionName: "lastCheckIn",
    query: LIVE,
  });
  const requiredDelay = useReadContract({
    address: appConfig.vdf,
    abi: vdfAbi,
    functionName: "requiredDelay",
    query: LIVE,
  });
  const inactivityConfirmed = useReadContract({
    address: appConfig.vdf,
    abi: vdfAbi,
    functionName: "inactivityConfirmed",
    query: LIVE,
  });

  const frostOwner = useReadContract({
    address: appConfig.frost,
    abi: frostAbi,
    functionName: "owner",
    query: LIVE,
  });
  const hasGroupKey = useReadContract({
    address: appConfig.frost,
    abi: frostAbi,
    functionName: "hasGroupKey",
    query: LIVE,
  });
  const groupPublicKeyX = useReadContract({
    address: appConfig.frost,
    abi: frostAbi,
    functionName: "groupPublicKeyX",
    query: LIVE,
  });
  const groupKeyPrefix = useReadContract({
    address: appConfig.frost,
    abi: frostAbi,
    functionName: "groupKeyPrefix",
    query: LIVE,
  });
  const groupKeyIsCompressed = useReadContract({
    address: appConfig.frost,
    abi: frostAbi,
    functionName: "groupKeyIsCompressed",
    query: LIVE,
  });

  const frostState = useQuery<FrostStateResponse>({
    queryKey: ["frost-state"],
    queryFn: async () => {
      const res = await fetch("/api/frost/state", { cache: "no-store" });
      if (!res.ok) {
        throw new Error(`frost state request failed (${res.status})`);
      }
      return (await res.json()) as FrostStateResponse;
    },
    refetchInterval: 10_000,
  });

  const refetch = () => {
    void block.refetch();
    void vaultBalance.refetch();
    void vaultOwner.refetch();
    void beneficiary.refetch();
    void beneficiarySet.refetch();
    void released.refetch();
    void vaultChallenge.refetch();
    void vaultLastCheckInBlock.refetch();
    void vaultLastCheckInTime.refetch();
    void vdfN.refetch();
    void vdfT.refetch();
    void vdfChallenge.refetch();
    void vdfLastCheckIn.refetch();
    void requiredDelay.refetch();
    void inactivityConfirmed.refetch();
    void frostOwner.refetch();
    void hasGroupKey.refetch();
    void groupPublicKeyX.refetch();
    void groupKeyPrefix.refetch();
    void groupKeyIsCompressed.refetch();
    void frostState.refetch();
  };

  const currentBlock = block.data;
  const blocksUntilEligible =
    currentBlock !== undefined &&
    vdfLastCheckIn.data !== undefined &&
    requiredDelay.data !== undefined
      ? vdfLastCheckIn.data + requiredDelay.data + 1n - currentBlock
      : undefined;

  const vdfEligible =
    !inactivityConfirmed.data &&
    blocksUntilEligible !== undefined &&
    blocksUntilEligible <= 0n;

  return {
    enabled: isConfigured,
    config: appConfig,
    block: currentBlock,
    blocksUntilEligible,
    vdfEligible,
    vault: {
      owner: vaultOwner.data,
      beneficiary: beneficiary.data,
      beneficiarySet: beneficiarySet.data,
      released: released.data,
      challenge: vaultChallenge.data,
      lastCheckInBlock: vaultLastCheckInBlock.data,
      lastCheckInTime: vaultLastCheckInTime.data,
      balance: vaultBalance.data?.value,
    },
    vdf: {
      n: vdfN.data,
      t: vdfT.data,
      challenge: vdfChallenge.data,
      lastCheckIn: vdfLastCheckIn.data,
      requiredDelay: requiredDelay.data,
      confirmed: inactivityConfirmed.data,
    },
    frost: {
      owner: frostOwner.data,
      hasGroupKey: hasGroupKey.data,
      x: groupPublicKeyX.data,
      prefix: groupKeyPrefix.data,
      compressed: groupKeyIsCompressed.data,
    },
    frostState: frostState.data,
    frostStateError: frostState.error
      ? (frostState.error as Error).message
      : undefined,
    isLoading:
      vaultOwner.isLoading ||
      vdfChallenge.isLoading ||
      block.isLoading,
    refetch,
  };
}
