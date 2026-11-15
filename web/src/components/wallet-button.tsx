"use client";

import {
  useAccount,
  useChainId,
  useConnect,
  useConnectors,
  useDisconnect,
  useSwitchChain,
} from "wagmi";

import { errorMessage } from "@/lib/errors";
import { shorten } from "@/lib/format";
import { targetChain } from "@/lib/wagmi";
import { Button } from "@/components/ui";

export function WalletButton() {
  const { address, isConnected, isConnecting } = useAccount();
  const connectors = useConnectors();
  const chainId = useChainId();
  const { mutate: connect, isPending: connecting, error: connectError } =
    useConnect();
  const { mutate: disconnect } = useDisconnect();
  const { mutate: switchChain, isPending: switching, error: switchError } =
    useSwitchChain();

  const wrongChain = isConnected && chainId !== targetChain.id;

  if (!isConnected) {
    const connector = connectors[0];
    return (
      <div className="flex flex-col items-end gap-1">
        <Button
          size="sm"
          busy={connecting || isConnecting || !connector}
          onClick={() => connector && connect({ connector })}
        >
          Connect MetaMask
        </Button>
        {!connector && (
          <span className="text-[11px] text-amber-300">
            No browser wallet detected
          </span>
        )}
        {connectError && (
          <span className="max-w-56 text-right text-[11px] text-rose-300">
            {errorMessage(connectError)}
          </span>
        )}
      </div>
    );
  }

  return (
    <div className="flex flex-col items-end gap-1">
      <div className="flex items-center gap-2">
        {wrongChain && (
          <Button
            size="sm"
            variant="secondary"
            busy={switching}
            onClick={() => switchChain({ chainId: targetChain.id })}
          >
            Switch to {targetChain.name}
          </Button>
        )}
        <span className="hidden rounded-lg border border-white/10 bg-white/5 px-2.5 py-1 font-mono text-xs text-slate-300 sm:inline">
          {shorten(address, 6, 4)}
        </span>
        <Button size="sm" variant="ghost" onClick={() => disconnect()}>
          Disconnect
        </Button>
      </div>
      {wrongChain && (
        <span className="text-[11px] text-amber-300">
          Wallet is on chain {chainId}, expected {targetChain.id}
        </span>
      )}
      {switchError && (
        <span className="max-w-56 text-right text-[11px] text-rose-300">
          {errorMessage(switchError)}
        </span>
      )}
    </div>
  );
}
