"use client";

import { useState } from "react";
import { useAccount } from "wagmi";
import { isAddress, parseEther } from "viem";

import type { DkgResponse } from "@/lib/api-types";
import { postJson } from "@/lib/client-api";
import { appConfig, isConfigured } from "@/lib/config";
import { frostAbi, groupKeyMatches, vaultAbi } from "@/lib/contracts";
import { errorMessage } from "@/lib/errors";
import { formatAmount, formatBlock, formatDate, shorten } from "@/lib/format";
import { useProtocolState } from "@/hooks/use-protocol";
import { useTx } from "@/hooks/use-tx";
import { DevMineButton } from "@/components/dev-mine-button";
import {
  Alert,
  Badge,
  Button,
  Card,
  CopyValue,
  Field,
  KeyValue,
  Stat,
  TextInput,
} from "@/components/ui";

export function OwnerDashboard() {
  const protocol = useProtocolState();
  const tx = useTx();
  const { address, isConnected } = useAccount();

  const [beneficiaryInput, setBeneficiaryInput] = useState("");
  const [depositInput, setDepositInput] = useState("0.1");
  const [trustees, setTrustees] = useState(5);
  const [threshold, setThreshold] = useState(3);
  const [dkg, setDkg] = useState<DkgResponse | null>(null);
  const [dkgBusy, setDkgBusy] = useState(false);
  const [dkgError, setDkgError] = useState<string | null>(null);
  const [formError, setFormError] = useState<string | null>(null);

  const owner = protocol.vault.owner;
  const isOwner = Boolean(
    address && owner && address.toLowerCase() === owner.toLowerCase(),
  );
  const candidateKey = dkg?.groupKey ?? protocol.frostState?.groupKey;
  const installedMatches = groupKeyMatches(
    candidateKey,
    protocol.frost.x,
    protocol.frost.prefix,
  );

  async function handleSetBeneficiary() {
    setFormError(null);
    if (!isAddress(beneficiaryInput)) {
      setFormError("Enter a valid 0x beneficiary address.");
      return;
    }
    const ok = await tx.run("Set beneficiary", async () => {
      await tx.write(
        {
          address: appConfig.vault,
          abi: vaultAbi,
          functionName: "setBeneficiary",
          args: [beneficiaryInput],
        },
        "Set beneficiary",
      );
      protocol.refetch();
    });
    if (ok) setBeneficiaryInput("");
  }

  async function handleCheckIn() {
    setFormError(null);
    await tx.run("Check in", async () => {
      // One transaction resets both clocks: the vault rotates its challenge
      // and calls VDFVerifier.checkIn() itself.
      await tx.write(
        { address: appConfig.vault, abi: vaultAbi, functionName: "checkIn" },
        "Vault check-in",
      );
      protocol.refetch();
    });
  }

  async function handleDeposit() {
    setFormError(null);
    let value: bigint;
    try {
      value = parseEther(depositInput);
    } catch {
      setFormError("Enter a valid ETH amount.");
      return;
    }
    if (value <= 0n) {
      setFormError("Deposit must be greater than zero.");
      return;
    }
    await tx.run("Deposit", async () => {
      await tx.send({ to: appConfig.vault, value }, "Deposit");
      protocol.refetch();
    });
  }

  async function handleDkg() {
    setDkgBusy(true);
    setDkgError(null);
    try {
      const result = await postJson<DkgResponse>("/api/frost/dkg", {
        trustees,
        threshold,
      });
      setDkg(result);
      protocol.refetch();
    } catch (cause) {
      setDkgError(errorMessage(cause));
    } finally {
      setDkgBusy(false);
    }
  }

  async function handleInstallKey() {
    if (!candidateKey) return;
    setFormError(null);
    await tx.run("Install FROST key", async () => {
      await tx.write(
        {
          address: appConfig.frost,
          abi: frostAbi,
          functionName: "setGroupPublicKeyCompressed",
          args: [candidateKey],
        },
        "Install FROST key",
      );
      protocol.refetch();
    });
  }

  return (
    <div className="space-y-6">
      {!isConfigured && (
        <Alert tone="danger" title="Contracts are not configured">
          Deploy the local stack first: run{" "}
          <code className="font-mono">web/scripts/deploy-local.sh</code>, then
          restart <code className="font-mono">pnpm dev</code>.
        </Alert>
      )}

      {isConfigured && !isConnected && (
        <Alert tone="info" title="Connect the owner wallet">
          Use the Connect button in the header. On Anvil, import the deployer
          account (account #0) into MetaMask so this page can sign owner-only
          transactions.
        </Alert>
      )}

      {isConnected && !isOwner && (
        <Alert tone="warning" title="This wallet is not the vault owner">
          Connected {shorten(address, 8, 6)}; the vault owner is{" "}
          {shorten(owner, 8, 6)}. Owner-only actions will revert. Import the
          Anvil deployer key or redeploy with a different owner.
        </Alert>
      )}

      {isOwner && (
        <Alert tone="success" title="Owner wallet connected">
          Check-ins, beneficiary changes and the FROST group key are available.
        </Alert>
      )}

      {(tx.note || tx.error) && (
        <Alert tone={tx.error ? "danger" : "info"} title={tx.error ? "Transaction failed" : "Transaction"}>
          {tx.error ?? tx.note}
        </Alert>
      )}

      {formError && <Alert tone="danger">{formError}</Alert>}

      <div className="grid gap-6 lg:grid-cols-2">
        <Card
          title="Vault state"
          description="The on-chain will, its funds, and the challenge that release signatures are bound to."
          actions={
            protocol.vault.released ? (
              <Badge tone="danger">Released</Badge>
            ) : (
              <Badge tone="success">Active</Badge>
            )
          }
        >
          <dl>
            <KeyValue label="Owner">
              {owner ? <CopyValue value={owner} /> : "—"}
            </KeyValue>
            <KeyValue label="Beneficiary">
              {protocol.vault.beneficiarySet && protocol.vault.beneficiary ? (
                <CopyValue value={protocol.vault.beneficiary} />
              ) : (
                <Badge tone="warning">not set</Badge>
              )}
            </KeyValue>
            <KeyValue label="Balance">
              {formatAmount(protocol.vault.balance)} ETH
            </KeyValue>
            <KeyValue label="Vault challenge">
              {protocol.vault.challenge ? (
                <CopyValue value={protocol.vault.challenge} />
              ) : (
                "—"
              )}
            </KeyValue>
            <KeyValue label="Last check-in">
              {formatDate(protocol.vault.lastCheckInTime)} · block{" "}
              {formatBlock(protocol.vault.lastCheckInBlock)}
            </KeyValue>
          </dl>
        </Card>

        <Card
          title="Liveness check-in"
          description="One transaction resets both clocks. A pending trustee release signature becomes invalid after a check-in."
        >
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <Stat
              label="VDF check-in"
              value={formatBlock(protocol.vdf.lastCheckIn)}
              hint="block"
            />
            <Stat
              label="Delay"
              value={protocol.vdf.requiredDelay?.toString() ?? "—"}
              hint="blocks"
            />
            <Stat
              label="Until eligible"
              value={
                protocol.vdf.confirmed
                  ? "confirmed"
                  : protocol.blocksUntilEligible === undefined
                    ? "—"
                    : protocol.blocksUntilEligible <= 0n
                      ? "ready"
                      : protocol.blocksUntilEligible.toString()
              }
              hint="VDF proof window"
            />
            <Stat
              label="VDF status"
              value={
                protocol.vdf.confirmed ? (
                  <Badge tone="warning">Inactivity confirmed</Badge>
                ) : (
                  <Badge tone="success">Monitoring</Badge>
                )
              }
            />
          </div>

          <div className="mt-4 flex flex-wrap items-center gap-3">
            <Button
              busy={tx.busy}
              disabled={!isOwner || protocol.vault.released}
              onClick={() => void handleCheckIn()}
            >
              Check in
            </Button>
            {appConfig.chainId === 31337 && (
              <DevMineButton blocks={12} onMined={protocol.refetch} />
            )}
          </div>
          <p className="mt-3 text-xs leading-relaxed text-slate-500">
            The vault check-in rotates{" "}
            <code className="font-mono">vault.currentChallenge</code> and calls{" "}
            <code className="font-mono">vdfVerifier.checkIn()</code>, which rotates{" "}
            <code className="font-mono">vdf.currentChallenge</code> and resets the
            inactivity timer. The VDF clock cannot be reset by anyone else.
          </p>
        </Card>

        <Card
          title="Beneficiary"
          description="The address that receives the vault balance once the dual gate is satisfied."
        >
          <div className="space-y-3">
            <Field
              label="Beneficiary address"
              hint="Changes are allowed until release. Re-setting the exact same address reverts."
            >
              <TextInput
                value={beneficiaryInput}
                onChange={(event) => setBeneficiaryInput(event.target.value)}
                placeholder="0x0000000000000000000000000000000000000000"
                spellCheck={false}
              />
            </Field>
            <Button
              busy={tx.busy}
              disabled={!isOwner || protocol.vault.released}
              onClick={() => void handleSetBeneficiary()}
            >
              Set beneficiary
            </Button>
            {protocol.vault.beneficiarySet && protocol.vault.beneficiary && (
              <p className="text-xs text-slate-500">
                Currently{" "}
                <span className="font-mono">
                  {shorten(protocol.vault.beneficiary, 10, 8)}
                </span>
              </p>
            )}
          </div>
        </Card>

        <Card
          title="Fund the vault"
          description="Native ETH held by the vault is forwarded to the beneficiary when release succeeds."
        >
          <div className="space-y-3">
            <Field label="Amount (ETH)" hint="Any connected account may deposit.">
              <TextInput
                value={depositInput}
                onChange={(event) => setDepositInput(event.target.value)}
                inputMode="decimal"
                spellCheck={false}
              />
            </Field>
            <Button
              busy={tx.busy}
              disabled={!isConnected || protocol.vault.released}
              onClick={() => void handleDeposit()}
            >
              Deposit ETH
            </Button>
            <p className="text-xs text-slate-500">
              Vault balance: {formatAmount(protocol.vault.balance)} ETH
            </p>
          </div>
        </Card>

        <Card
          title="FROST trustee committee"
          description="Create the threshold key and install its group public key. The vault only accepts release signatures from this key."
          className="lg:col-span-2"
        >
          <div className="grid gap-6 lg:grid-cols-2">
            <div className="space-y-3">
              <h3 className="text-sm font-semibold text-slate-200">
                1. Run the DKG ceremony
              </h3>
              <div className="grid grid-cols-2 gap-3">
                <Field label="Trustees">
                  <TextInput
                    type="number"
                    min={2}
                    max={10}
                    value={trustees}
                    onChange={(event) => {
                      const next = Number(event.target.value);
                      setTrustees(next);
                      if (threshold > next) setThreshold(next);
                    }}
                  />
                </Field>
                <Field label="Threshold">
                  <TextInput
                    type="number"
                    min={2}
                    max={trustees}
                    value={threshold}
                    onChange={(event) => setThreshold(Number(event.target.value))}
                  />
                </Field>
              </div>
              <Button
                busy={dkgBusy}
                disabled={!isOwner || protocol.vault.released}
                onClick={() => void handleDkg()}
              >
                Run DKG
              </Button>
              {dkgError && (
                <Alert tone="danger" title="DKG failed">
                  {dkgError}
                </Alert>
              )}
              {dkg && (
                <Alert tone="success" title={`DKG complete — ${dkg.threshold}-of-${dkg.trustees}`}>
                  <div className="space-y-1">
                    <div className="font-mono text-[11px] break-all">
                      {dkg.groupKey}
                    </div>
                    <div>
                      Shares written for trustees {dkg.shares.join(", ")}. Use
                      the trustee page to produce a release signature.
                    </div>
                  </div>
                </Alert>
              )}
            </div>

            <div className="space-y-3">
              <h3 className="text-sm font-semibold text-slate-200">
                2. Install the group key on-chain
              </h3>
              <dl>
                <KeyValue label="Verifier key">
                  {protocol.frost.hasGroupKey ? (
                    <Badge tone="success">set</Badge>
                  ) : (
                    <Badge tone="warning">not set</Badge>
                  )}
                </KeyValue>
                <KeyValue label="Key x">
                  {protocol.frost.x !== undefined
                    ? shorten(protocol.frost.x.toString(16), 12, 8)
                    : "—"}
                </KeyValue>
                <KeyValue label="Prefix">
                  {protocol.frost.prefix !== undefined
                    ? `0x0${protocol.frost.prefix}`
                    : "—"}
                </KeyValue>
                <KeyValue label="Stored shares">
                  {protocol.frostState?.configured
                    ? `${protocol.frostState.threshold}-of-${protocol.frostState.trustees}`
                    : "none yet"}
                </KeyValue>
              </dl>
              {candidateKey && (
                <div className="rounded-lg border border-white/5 bg-slate-950/40 px-3 py-2">
                  <div className="text-xs uppercase tracking-wide text-slate-500">
                    Candidate group key
                  </div>
                  <CopyValue value={candidateKey} />
                </div>
              )}
              <div className="flex flex-wrap items-center gap-2">
                <Button
                  busy={tx.busy}
                  disabled={
                    !isOwner ||
                    !candidateKey ||
                    installedMatches ||
                    Boolean(protocol.frost.hasGroupKey)
                  }
                  onClick={() => void handleInstallKey()}
                >
                  {installedMatches
                    ? "Key already installed"
                    : protocol.frost.hasGroupKey
                      ? "Key locked — redeploy to change"
                      : "Install group key"}
                </Button>
                {candidateKey &&
                  (installedMatches ? (
                    <Badge tone="success">matches stored shares</Badge>
                  ) : (
                    <Badge tone="warning">not installed</Badge>
                  ))}
              </div>
              {protocol.frost.hasGroupKey && !installedMatches && (
                <Alert tone="danger" title="Installed key cannot be changed">
                  The verifier key is one-shot. Re-run{" "}
                  <code className="font-mono">
                    web/scripts/deploy-local.sh
                  </code>{" "}
                  and the DKG ceremony to install a matching key.
                </Alert>
              )}
            </div>
          </div>

          <div className="mt-4">
            <Alert tone="warning" title="Local demo custody model">
              This API runs the DKG and writes every trustee share to{" "}
              <code className="font-mono">web/.frost/shares</code> so the demo
              can sign from one machine. In a real deployment each trustee
              generates and keeps only its own share, and the website never sees
              the others.
            </Alert>
          </div>
        </Card>
      </div>
    </div>
  );
}
