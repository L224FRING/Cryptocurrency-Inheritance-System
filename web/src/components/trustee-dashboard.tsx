"use client";

import { useMemo, useState } from "react";
import { useAccount } from "wagmi";

import type { SignResponse, VdfProveResponse } from "@/lib/api-types";
import { postJson } from "@/lib/client-api";
import { appConfig, isConfigured } from "@/lib/config";
import { computeReleaseHash, groupKeyMatches, vdfAbi, vaultAbi } from "@/lib/contracts";
import { errorMessage } from "@/lib/errors";
import { formatAmount, formatBlock, shorten } from "@/lib/format";
import { useProtocolState } from "@/hooks/use-protocol";
import { useTx } from "@/hooks/use-tx";
import { DevMineButton } from "@/components/dev-mine-button";
import {
  Alert,
  Badge,
  Button,
  Card,
  CopyValue,
  KeyValue,
  Stat,
} from "@/components/ui";

/** The API response plus the hash it was produced for, so we can detect a
 * stale signature after the owner rotates the challenge. */
type SignedRelease = SignResponse & { releaseHash: `0x${string}` };

export function TrusteeDashboard() {
  const protocol = useProtocolState();
  const tx = useTx();
  const { address, isConnected } = useAccount();

  const [proof, setProof] = useState<VdfProveResponse | null>(null);
  const [proofBusy, setProofBusy] = useState(false);
  const [proofError, setProofError] = useState<string | null>(null);

  const [signed, setSigned] = useState<SignedRelease | null>(null);
  const [signBusy, setSignBusy] = useState(false);
  const [signError, setSignError] = useState<string | null>(null);

  const { vault, vdf, frost, frostState } = protocol;

  const releaseHash = useMemo(() => {
    if (!vault.beneficiary || !vault.challenge) return undefined;
    return computeReleaseHash(
      appConfig.vault,
      vault.beneficiary,
      vault.challenge,
      BigInt(appConfig.chainId),
    );
  }, [vault.beneficiary, vault.challenge]);

  // A proof or signature is only valid for the challenge it was produced
  // against. Invalidate stale artifacts when the owner checks in.
  const proofMatchesChallenge =
    proof !== null &&
    vdf.challenge !== undefined &&
    BigInt(proof.x) === vdf.challenge;
  const signatureMatchesChallenge =
    signed !== null &&
    releaseHash !== undefined &&
    signed.releaseHash === releaseHash;
  const signatureKeyMatches =
    signed !== null && groupKeyMatches(signed.groupKey, frost.x, frost.prefix);

  const gates = {
    beneficiary: Boolean(vault.beneficiarySet && vault.beneficiary),
    groupKey: Boolean(frost.hasGroupKey),
    vdfConfirmed: Boolean(vdf.confirmed),
    notReleased: vault.released === false,
  };
  const readyToRelease =
    gates.beneficiary &&
    gates.groupKey &&
    gates.vdfConfirmed &&
    gates.notReleased &&
    signatureMatchesChallenge &&
    signatureKeyMatches;

  async function handleProve() {
    if (vdf.challenge === undefined || vdf.t === undefined || vdf.n === undefined) {
      return;
    }
    setProofBusy(true);
    setProofError(null);
    setProof(null);
    try {
      const result = await postJson<VdfProveResponse>("/api/vdf/prove", {
        challenge: vdf.challenge.toString(),
        t: Number(vdf.t),
        n: vdf.n.toString(),
      });
      setProof(result);
    } catch (cause) {
      setProofError(errorMessage(cause));
    } finally {
      setProofBusy(false);
    }
  }

  async function handleSubmitProof() {
    if (!proof) return;
    await tx.run("Submit VDF proof", async () => {
      await tx.write(
        {
          address: appConfig.vdf,
          abi: vdfAbi,
          functionName: "submitVDFProof",
          args: [BigInt(proof.y), proof.proofPoints.map((point) => BigInt(point))],
        },
        "Submit VDF proof",
      );
      protocol.refetch();
    });
  }

  async function handleSign() {
    if (!releaseHash) return;
    setSignBusy(true);
    setSignError(null);
    setSigned(null);
    try {
      const result = await postJson<SignResponse>("/api/frost/sign", {
        releaseHash,
      });
      setSigned({ ...result, releaseHash });
    } catch (cause) {
      setSignError(errorMessage(cause));
    } finally {
      setSignBusy(false);
    }
  }

  async function handleRelease() {
    if (!signed) return;
    await tx.run("Release vault", async () => {
      await tx.write(
        {
          address: appConfig.vault,
          abi: vaultAbi,
          functionName: "release",
          args: [signed.signature],
        },
        "Release vault",
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
        <Alert tone="info" title="Connect a wallet to submit transactions">
          The trustee flow is permissionless on-chain: any account may submit
          the VDF proof and the release transaction once the gates are met. The
          threshold signature itself comes from the trustee shares held by the
          local FROST service.
        </Alert>
      )}

      {isConnected && (
        <Alert tone="neutral" title={`Acting as ${shorten(address, 8, 6)}`}>
          This account only pays gas. The release signature is produced by the
          threshold committee configured on the owner page.
        </Alert>
      )}

      {(tx.note || tx.error) && (
        <Alert
          tone={tx.error ? "danger" : "info"}
          title={tx.error ? "Transaction failed" : "Transaction"}
        >
          {tx.error ?? tx.note}
        </Alert>
      )}

      <Card
        title="Release gates"
        description="The vault only pays the beneficiary when the double gate is open: confirmed inactivity plus a valid threshold signature over the current challenge."
        actions={
          vault.released ? (
            <Badge tone="danger">Released</Badge>
          ) : readyToRelease ? (
            <Badge tone="success">Ready to release</Badge>
          ) : (
            <Badge tone="warning">Gates closed</Badge>
          )
        }
      >
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          <Stat
            label="Beneficiary"
            value={
              gates.beneficiary ? (
                <Badge tone="success">set</Badge>
              ) : (
                <Badge tone="danger">missing</Badge>
              )
            }
            hint={vault.beneficiary ? shorten(vault.beneficiary) : "owner must set"}
          />
          <Stat
            label="FROST key"
            value={
              gates.groupKey ? (
                <Badge tone="success">installed</Badge>
              ) : (
                <Badge tone="danger">missing</Badge>
              )
            }
            hint={
              frostState?.configured
                ? `${frostState.threshold}-of-${frostState.trustees} shares`
                : "run DKG on owner page"
            }
          />
          <Stat
            label="VDF inactivity"
            value={
              gates.vdfConfirmed ? (
                <Badge tone="success">confirmed</Badge>
              ) : (
                <Badge tone="warning">monitoring</Badge>
              )
            }
            hint={
              protocol.blocksUntilEligible === undefined
                ? "proof window"
                : protocol.blocksUntilEligible <= 0n
                  ? "proof accepted now"
                  : `${protocol.blocksUntilEligible} blocks left`
            }
          />
          <Stat
            label="Vault balance"
            value={`${formatAmount(vault.balance)} ETH`}
            hint={vault.released ? "already paid out" : "held by contract"}
          />
        </div>
      </Card>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card
          title="1. Prove inactivity"
          description="Run the VDF over vdf.currentChallenge, then submit y and the Pietrzak proof points."
          actions={
            vdf.confirmed ? (
              <Badge tone="success">done</Badge>
            ) : (
              <Badge tone="neutral">pending</Badge>
            )
          }
        >
          <dl className="mb-4">
            <KeyValue label="Challenge">
              {vdf.challenge !== undefined ? (
                <CopyValue value={vdf.challenge.toString()} />
              ) : (
                "—"
              )}
            </KeyValue>
            <KeyValue label="T (squarings)">{vdf.t?.toString() ?? "—"}</KeyValue>
            <KeyValue label="N (modulus)">
              {vdf.n !== undefined ? shorten(vdf.n.toString(), 12, 8) : "—"}
            </KeyValue>
            <KeyValue label="Last VDF check-in">
              {formatBlock(vdf.lastCheckIn)}
            </KeyValue>
          </dl>

          {!vdf.confirmed &&
            protocol.blocksUntilEligible !== undefined &&
            protocol.blocksUntilEligible > 0n && (
              <Alert tone="warning" title="Too early to submit">
                The contract requires{" "}
                <code className="font-mono">
                  block.number &gt; lastCheckIn + requiredDelay
                </code>
                . Wait {protocol.blocksUntilEligible.toString()} more block(s),
                or advance Anvil below.
              </Alert>
            )}

          <div className="mt-3 flex flex-wrap items-center gap-2">
            <Button
              variant="secondary"
              busy={proofBusy}
              disabled={
                vdf.confirmed ||
                vdf.challenge === undefined ||
                vdf.t === undefined ||
                vdf.n === undefined
              }
              onClick={() => void handleProve()}
            >
              Generate proof
            </Button>
            {!vdf.confirmed && appConfig.chainId === 31337 && (
              <DevMineButton blocks={12} onMined={protocol.refetch} />
            )}
          </div>

          {proofError && (
            <div className="mt-3">
              <Alert tone="danger" title="Prover failed">
                {proofError}
              </Alert>
            </div>
          )}

          {proof && (
            <div className="mt-3 space-y-2">
              <Alert
                tone={proofMatchesChallenge ? "success" : "warning"}
                title={
                  proofMatchesChallenge
                    ? "Proof generated"
                    : "Proof is for a stale challenge"
                }
              >
                {proofMatchesChallenge
                  ? "Submit it to confirm inactivity."
                  : "The owner checked in after this proof was generated. Generate a fresh one."}
              </Alert>
              <dl>
                <KeyValue label="y">
                  <CopyValue value={proof.y} />
                </KeyValue>
                <KeyValue label="Proof points">
                  {proof.proofPoints.length}
                </KeyValue>
              </dl>
              <Button
                busy={tx.busy}
                disabled={!proofMatchesChallenge || vdf.confirmed}
                onClick={() => void handleSubmitProof()}
              >
                Submit VDF proof
              </Button>
            </div>
          )}
        </Card>

        <Card
          title="2. Collect the threshold signature"
          description="Ask the trustee committee to sign the vault's release hash. Any t-of-n subset of the stored shares is enough."
          actions={
            signatureMatchesChallenge ? (
              <Badge tone="success">signed</Badge>
            ) : (
              <Badge tone="neutral">unsigned</Badge>
            )
          }
        >
          <dl className="mb-4">
            <KeyValue label="Vault challenge">
              {vault.challenge ? (
                <CopyValue value={vault.challenge} />
              ) : (
                "—"
              )}
            </KeyValue>
            <KeyValue label="Beneficiary">
              {vault.beneficiary ? (
                <CopyValue value={vault.beneficiary} />
              ) : (
                <Badge tone="warning">not set</Badge>
              )}
            </KeyValue>
            <KeyValue label="Release hash">
              {releaseHash ? <CopyValue value={releaseHash} /> : "—"}
            </KeyValue>
          </dl>

          <Button
            busy={signBusy}
            disabled={!releaseHash || vault.released === true}
            onClick={() => void handleSign()}
          >
            Sign with FROST committee
          </Button>

          {signError && (
            <div className="mt-3">
              <Alert tone="danger" title="Signing failed">
                {signError}
              </Alert>
            </div>
          )}

          {signed && (
            <div className="mt-3 space-y-2">
              <Alert
                tone={
                  signatureMatchesChallenge && signatureKeyMatches
                    ? "success"
                    : "warning"
                }
                title={
                  !signatureKeyMatches
                    ? "Committee key is not installed"
                    : signatureMatchesChallenge
                      ? `Signed by trustees ${signed.participants.join(", ")}`
                      : "Signature is for a stale challenge"
                }
              >
                {!signatureKeyMatches
                  ? "The shares sign with a different group key than the verifier holds. Re-run DKG and install the matching key from the owner page."
                  : signatureMatchesChallenge
                    ? "The signature is bound to the current vault challenge."
                    : "Vault state changed after signing; collect a fresh signature."}
              </Alert>
              <dl>
                <KeyValue label="Signature">
                  <CopyValue value={signed.signature} />
                </KeyValue>
                <KeyValue label="Group key">
                  <CopyValue value={signed.groupKey} />
                </KeyValue>
              </dl>
            </div>
          )}
        </Card>

        <Card
          title="3. Release the vault"
          description="Verifies the threshold signature on-chain and forwards the vault balance to the beneficiary."
          className="lg:col-span-2"
          actions={
            vault.released ? (
              <Badge tone="danger">Released</Badge>
            ) : (
              <Badge tone="warning">Not released</Badge>
            )
          }
        >
          <div className="flex flex-wrap items-center gap-3">
            <Button
              busy={tx.busy}
              disabled={!readyToRelease || !isConnected}
              onClick={() => void handleRelease()}
            >
              Call vault.release()
            </Button>
            <span className="text-xs text-slate-500">
              {vault.released
                ? "The vault has already paid out."
                : !gates.vdfConfirmed
                  ? "Confirm VDF inactivity first."
                  : !signatureMatchesChallenge
                    ? "Collect a fresh threshold signature first."
                    : !signatureKeyMatches
                      ? "Install the matching FROST group key from the owner page."
                      : "All gates are open."}
            </span>
          </div>

          {vault.released && (
            <div className="mt-3">
              <Alert tone="success" title="Vault released">
                The balance was forwarded to{" "}
                {vault.beneficiary ? shorten(vault.beneficiary, 10, 8) : "the beneficiary"}.
              </Alert>
            </div>
          )}

          <div className="mt-4">
            <Alert tone="warning" title="Demo custody model">
              The API holds all shares in{" "}
              <code className="font-mono">web/.frost/shares</code> so one machine
              can drive the whole committee. In production each trustee would run
              its own client, keep only its own share, and approve the release
              independently.
            </Alert>
          </div>
        </Card>
      </div>
    </div>
  );
}
