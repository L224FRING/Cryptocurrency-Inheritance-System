import { spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

import { runCommand } from "@/lib/server/proc";

export type FrostState = {
  trustees: number;
  threshold: number;
  groupKey: `0x${string}`;
  session: `0x${string}`;
  createdAt: string;
  shares: number[];
  perTrusteeGroupKey?: Record<string, string>;
};

type DkgOutput = {
  session: string;
  trustees: number;
  threshold: number;
  group_verifying_key: string;
  per_trustee_group_key?: Record<string, string>;
};

type SignPartyOutput = {
  role: "aggregator" | "participant";
  signature?: string;
  group_verifying_key: string;
};

type VdfOutput = {
  x: string;
  y: string;
  t: number;
  n: string;
  proof_points: string[];
};

function frostPaths() {
  const webRoot = process.cwd();
  const repoRoot = process.env.FROST_REPO_ROOT ?? path.resolve(webRoot, "..");
  const stateDir = process.env.FROST_STATE_DIR ?? path.join(webRoot, ".frost");
  return {
    serviceBin:
      process.env.FROST_SERVICE_BIN ??
      path.join(repoRoot, "rust/frost-service/target/debug/frost-service"),
    relayBin:
      process.env.FROST_RELAY_BIN ??
      path.join(repoRoot, "rust/frost-service/target/debug/frost-relay"),
    stateDir,
    sharesDir: path.join(stateDir, "shares"),
    stateFile: path.join(stateDir, "state.json"),
  };
}

function assertBinaries() {
  const paths = frostPaths();
  if (!existsSync(/*turbopackIgnore: true*/ paths.serviceBin)) {
    throw new Error(
      `frost-service binary not found at ${paths.serviceBin}. Build it with: cargo build --manifest-path rust/frost-service/Cargo.toml`,
    );
  }
  if (!existsSync(/*turbopackIgnore: true*/ paths.relayBin)) {
    throw new Error(
      `frost-relay binary not found at ${paths.relayBin}. Build the crate first.`,
    );
  }
}

function with0x(value: string): `0x${string}` {
  return value.startsWith("0x") ? (value as `0x${string}`) : `0x${value}`;
}

function toWordHex(value: string): string {
  const big = BigInt(value);
  return `0x${big.toString(16).padStart(64, "0")}`;
}

function toQuantityHex(value: string): string {
  const big = BigInt(value);
  let hex = big.toString(16);
  if (hex.length % 2 !== 0) hex = `0${hex}`;
  return `0x${hex}`;
}

export function frostHealth() {
  const paths = frostPaths();
  return {
    serviceBin: paths.serviceBin,
    relayBin: paths.relayBin,
    serviceBinExists: existsSync(/*turbopackIgnore: true*/ paths.serviceBin),
    relayBinExists: existsSync(/*turbopackIgnore: true*/ paths.relayBin),
    stateDir: paths.stateDir,
    hasShares: shareIndices().length > 0,
  };
}

export function readState(): FrostState | null {
  const { stateFile } = frostPaths();
  if (!existsSync(/*turbopackIgnore: true*/ stateFile)) return null;
  try {
    return JSON.parse(readFileSync(/*turbopackIgnore: true*/ stateFile, "utf8")) as FrostState;
  } catch {
    return null;
  }
}

export function shareIndices(): number[] {
  const { sharesDir } = frostPaths();
  if (!existsSync(/*turbopackIgnore: true*/ sharesDir)) return [];
  return readdirSync(/*turbopackIgnore: true*/ sharesDir)
    .flatMap((name) => {
      const match = name.match(/^share-(\d+)\.json$/);
      return match ? [Number(match[1])] : [];
    })
    .sort((a, b) => a - b);
}

/** DKG and signing both touch the relay/share directory; run them one at a time. */
let queue: Promise<unknown> = Promise.resolve();
function serialize<T>(task: () => Promise<T>): Promise<T> {
  const result = queue.then(task, task);
  queue = result.then(
    () => undefined,
    () => undefined,
  );
  return result;
}

export function runDkg(trustees: number, threshold: number): Promise<FrostState> {
  return serialize(async () => {
    assertBinaries();
    const paths = frostPaths();
    rmSync(/*turbopackIgnore: true*/ paths.sharesDir, { recursive: true, force: true });
    mkdirSync(/*turbopackIgnore: true*/ paths.sharesDir, { recursive: true });

    const result = await runCommand(
      paths.serviceBin,
      [
        "dkg",
        "--trustees",
        String(trustees),
        "--threshold",
        String(threshold),
        "--out-dir",
        paths.sharesDir,
      ],
      { timeoutMs: 120_000 },
    );

    if (result.code !== 0) {
      throw new Error(
        `FROST DKG failed: ${(result.stderr || result.stdout).trim() || `exit ${result.code}`}`,
      );
    }

    const output = JSON.parse(result.stdout) as DkgOutput;
    const shares = shareIndices();
    if (shares.length !== trustees) {
      throw new Error(
        `DKG wrote ${shares.length} shares, expected ${trustees}`,
      );
    }

    const state: FrostState = {
      trustees,
      threshold,
      groupKey: with0x(output.group_verifying_key),
      session: with0x(output.session),
      createdAt: new Date().toISOString(),
      shares,
      perTrusteeGroupKey: output.per_trustee_group_key,
    };

    mkdirSync(/*turbopackIgnore: true*/ paths.stateDir, { recursive: true });
    const temp = `${paths.stateFile}.tmp`;
    writeFileSync(/*turbopackIgnore: true*/ temp, JSON.stringify(state, null, 2));
    renameSync(/*turbopackIgnore: true*/ temp, paths.stateFile);
    return state;
  });
}

function normalizeParticipants(
  requested: number[] | undefined,
  state: FrostState,
): number[] {
  const available = new Set(state.shares);
  const chosen =
    requested && requested.length > 0
      ? requested
      : state.shares.slice(0, state.threshold);
  const unique = Array.from(new Set(chosen)).sort((a, b) => a - b);
  if (unique.length < state.threshold) {
    throw new Error(
      `Need at least ${state.threshold} participants; got ${unique.length}`,
    );
  }
  for (const index of unique) {
    if (!available.has(index)) {
      throw new Error(`Trustee ${index} has no persisted share`);
    }
  }
  return unique;
}

type RunningRelay = { url: string; stop: () => void };

function startRelay(trustees: number): Promise<RunningRelay> {
  const { relayBin } = frostPaths();
  return new Promise((resolve, reject) => {
    const child = spawn(
      /*turbopackIgnore: true*/ relayBin,
      ["--listen", "127.0.0.1:0", "--trustees", String(trustees)],
      { stdio: ["ignore", "pipe", "pipe"] },
    );

    let buffer = "";
    let settled = false;
    const timer = setTimeout(() => {
      if (settled) return;
      settled = true;
      child.kill("SIGKILL");
      reject(new Error("frost-relay did not report a listening address"));
    }, 5_000);
    timer.unref?.();

    child.stdout.on("data", (chunk: Buffer) => {
      buffer += chunk.toString();
      const match = buffer.match(
        /frost-relay listening on (\S+) for trustees/,
      );
      if (match && !settled) {
        settled = true;
        clearTimeout(timer);
        resolve({
          url: `http://${match[1]}`,
          stop: () => {
            child.kill("SIGTERM");
          },
        });
      }
    });
    child.stderr.on("data", (chunk: Buffer) => {
      buffer += chunk.toString();
    });
    child.on("error", (cause) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      reject(cause);
    });
    child.on("exit", (code) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      reject(
        new Error(`frost-relay exited with ${code}: ${buffer.trim()}`),
      );
    });
  });
}

export type SignResult = {
  signature: `0x${string}`;
  groupKey: `0x${string}`;
  participants: number[];
  session: `0x${string}`;
};

export function runSign(
  releaseHash: string,
  requestedParticipants?: number[],
): Promise<SignResult> {
  return serialize(async () => {
    assertBinaries();
    const state = readState();
    if (!state) {
      throw new Error(
        "No FROST shares are stored. Run the DKG ceremony from the owner page first.",
      );
    }

    const message = releaseHash.startsWith("0x")
      ? releaseHash
      : `0x${releaseHash}`;
    if (!/^0x[0-9a-fA-F]{64}$/.test(message)) {
      throw new Error("releaseHash must be a 32-byte hex value");
    }

    const participants = normalizeParticipants(requestedParticipants, state);
    const session = with0x(randomBytes(32).toString("hex"));
    const relay = await startRelay(state.trustees);

    try {
      const results = await Promise.all(
        participants.map(async (index, position) => {
          const args = [
            "sign-party",
            "--index",
            String(index),
            "--share",
            path.join(frostPaths().sharesDir, `share-${index}.json`),
            "--relay",
            relay.url,
            "--participants",
            participants.join(","),
            "--session",
            session,
            "--raw-message",
            "--message",
            message,
            "--timeout-ms",
            "30000",
          ];
          if (position === 0) args.push("--aggregate");

          const result = await runCommand(frostPaths().serviceBin, args, {
            timeoutMs: 45_000,
          });
          return { index, result };
        }),
      );

      const aggregator = results[0];
      if (aggregator.result.code !== 0) {
        const detail =
          aggregator.result.stderr.trim() ||
          aggregator.result.stdout.trim() ||
          `aggregator exited ${aggregator.result.code}`;
        throw new Error(`FROST signing failed: ${detail}`);
      }

      const output = JSON.parse(aggregator.result.stdout) as SignPartyOutput;
      if (!output.signature) {
        throw new Error("FROST aggregator did not return a signature");
      }

      return {
        signature: with0x(output.signature),
        groupKey: with0x(output.group_verifying_key),
        participants,
        session,
      };
    } finally {
      relay.stop();
    }
  });
}

export type VdfResult = {
  x: `0x${string}`;
  y: `0x${string}`;
  proofPoints: string[];
  t: number;
  n: `0x${string}`;
};

export function runVdf(
  challenge: string,
  t: number,
  n: string,
): Promise<VdfResult> {
  return serialize(async () => {
    assertBinaries();
    const result = await runCommand(
      frostPaths().serviceBin,
      [
        "vdf",
        "--t",
        String(t),
        "--n",
        toQuantityHex(n),
        "--input",
        toWordHex(challenge),
      ],
      { timeoutMs: 180_000 },
    );

    if (result.code !== 0) {
      throw new Error(
        `VDF prover failed: ${(result.stderr || result.stdout).trim() || `exit ${result.code}`}`,
      );
    }

    const output = JSON.parse(result.stdout) as VdfOutput;
    return {
      x: with0x(output.x),
      y: with0x(output.y),
      proofPoints: output.proof_points,
      t: output.t,
      n: with0x(output.n),
    };
  });
}
