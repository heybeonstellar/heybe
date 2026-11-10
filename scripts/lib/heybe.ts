// Shared helpers for the Heybe testnet flows.
//
// Runs on Node 22.18+ with built-in TypeScript type stripping (no build step).
// Secrets come from local stellar CLI identities and are never printed.

import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  type Account,
  type Transaction,
  BASE_FEE,
  Contract,
  Keypair,
  Operation,
  TransactionBuilder,
  authorizeEntry,
  buildWithDelegatesEntry,
  inspectAuthEntry,
  nativeToScVal,
  rpc,
  scValToNative,
  xdr,
} from "@stellar/stellar-sdk";

const here = dirname(fileURLToPath(import.meta.url));
export const ROOT = join(here, "..", "..");

export type Deployments = {
  network: string;
  rpcUrl: string;
  networkPassphrase: string;
  accountWasmHash: string;
  registry: string;
  testNft: string;
  xlmSac: string;
  stellar8004IdentityRegistry: string;
};

export const deployments: Deployments = JSON.parse(
  readFileSync(join(ROOT, "deployments", "testnet.json"), "utf8"),
);
export const server = new rpc.Server(deployments.rpcUrl);
export const passphrase = deployments.networkPassphrase;

/** Loads a stellar CLI identity as a Keypair. */
export function identity(name: string): Keypair {
  const secret = execFileSync("stellar", ["keys", "secret", name], {
    encoding: "utf8",
  }).trim();
  return Keypair.fromSecret(secret);
}

// ---------------------------------------------------------------------------
// ScVal helpers
// ---------------------------------------------------------------------------

export const addr = (a: string) => nativeToScVal(a, { type: "address" });
export const u32 = (n: number) => nativeToScVal(n, { type: "u32" });
export const i128 = (n: bigint) => nativeToScVal(n, { type: "i128" });
export const sym = (s: string) => nativeToScVal(s, { type: "symbol" });
export const str = (s: string) => nativeToScVal(s, { type: "string" });
export const vec = (items: xdr.ScVal[]) => nativeToScVal(items);

/** XLM has 7 decimals on Stellar. */
export const xlm = (n: number) => BigInt(Math.round(n * 10_000_000));

// ---------------------------------------------------------------------------
// Transactions
// ---------------------------------------------------------------------------

export type TxResult = { hash: string; value: unknown };

function buildCall(source: Account, contractId: string, method: string, args: xdr.ScVal[]): Transaction {
  return new TransactionBuilder(source, { fee: BASE_FEE, networkPassphrase: passphrase })
    .addOperation(new Contract(contractId).call(method, ...args))
    .setTimeout(120)
    .build();
}

async function send(tx: Transaction, signer: Keypair) {
  tx.sign(signer);
  const sent = await server.sendTransaction(tx);
  if (sent.status === "ERROR") {
    throw new Error(`send failed: ${JSON.stringify(sent.errorResult ?? sent)}`);
  }
  const res = await server.pollTransaction(sent.hash, { attempts: 40 });
  return { hash: sent.hash, res };
}

async function submit(tx: Transaction, signer: Keypair): Promise<TxResult> {
  const { hash, res } = await send(tx, signer);
  if (res.status !== "SUCCESS") {
    throw new Error(`transaction ${hash} ended with ${res.status}`);
  }
  const value = "returnValue" in res && res.returnValue ? scValToNative(res.returnValue) : undefined;
  return { hash, value };
}

/**
 * Submits a call that the network must reject, and returns the failed
 * transaction's hash. Simulation runs in recording mode, so it succeeds and
 * lists the signature the call really needs; `signer` cannot provide it, and
 * the transaction fails on chain. This leaves public proof of the rejection.
 */
export async function expectFailedOnChain(
  signer: Keypair,
  contractId: string,
  method: string,
  args: xdr.ScVal[],
): Promise<TxResult & { requiredSigner: string | null }> {
  const tx = buildCall(await server.getAccount(signer.publicKey()), contractId, method, args);
  const sim = await server.simulateTransaction(tx);
  if (rpc.Api.isSimulationError(sim)) {
    throw new SimulationFailed(method, sim.error);
  }
  const required = (sim.result?.auth ?? [])
    .map((e) => inspectAuthEntry(e).address)
    .find((a) => a !== null && a !== signer.publicKey()) ?? null;
  const { hash, res } = await send(rpc.assembleTransaction(tx, sim).build(), signer);
  if (res.status !== "FAILED") {
    throw new Error(`transaction ${hash} was expected to fail but ended with ${res.status}`);
  }
  return { hash, value: undefined, requiredSigner: required };
}

/** Simulates, assembles, signs and submits a contract call. */
export async function invoke(
  signer: Keypair,
  contractId: string,
  method: string,
  args: xdr.ScVal[],
): Promise<TxResult> {
  const source = await server.getAccount(signer.publicKey());
  const tx = buildCall(source, contractId, method, args);
  const sim = await server.simulateTransaction(tx);
  if (rpc.Api.isSimulationError(sim)) {
    throw new SimulationFailed(method, sim.error);
  }
  return submit(rpc.assembleTransaction(tx, sim).build(), signer);
}

/** Read-only call through simulation. */
export async function read(contractId: string, method: string, args: xdr.ScVal[], as: Keypair): Promise<unknown> {
  const source = await server.getAccount(as.publicKey());
  const sim = await server.simulateTransaction(buildCall(source, contractId, method, args));
  if (rpc.Api.isSimulationError(sim)) {
    throw new SimulationFailed(method, sim.error);
  }
  return sim.result?.retval ? scValToNative(sim.result.retval) : undefined;
}

export class SimulationFailed extends Error {
  readonly detail: string;
  constructor(method: string, detail: string) {
    super(`simulation of ${method} failed: ${shortError(detail)}`);
    this.detail = detail;
  }
}

/** Simulates a call that must be rejected and returns the reason. */
export async function expectRejected(
  signer: Keypair,
  contractId: string,
  method: string,
  args: xdr.ScVal[],
): Promise<string> {
  try {
    await invoke(signer, contractId, method, args);
  } catch (e) {
    if (e instanceof SimulationFailed) return shortError(e.detail);
    throw e;
  }
  throw new Error(`${method} was expected to be rejected but succeeded`);
}

/**
 * Path B (CAP-71). Submits a call in which `accountId` must authorize, with
 * `delegate` (a G key) signing as the account's delegated signer. `source`
 * only pays the fee.
 *
 * Two simulations, as CAP-71 describes: the first records the account's auth
 * entry, the second runs __check_auth against the signed entry.
 */
export async function invokeDelegated(
  source: Keypair,
  delegate: Keypair,
  accountId: string,
  contractId: string,
  method: string,
  args: xdr.ScVal[],
): Promise<TxResult> {
  const first = buildCall(await server.getAccount(source.publicKey()), contractId, method, args);
  const recorded = await server.simulateTransaction(first);
  if (rpc.Api.isSimulationError(recorded)) {
    throw new SimulationFailed(method, recorded.error);
  }

  const latest = await server.getLatestLedger();
  const validUntil = latest.sequence + 60;
  const auth = await Promise.all(
    (recorded.result?.auth ?? []).map(async (entry) => {
      if (inspectAuthEntry(entry).address !== accountId) return entry;
      const withDelegates = buildWithDelegatesEntry({
        entry,
        validUntilLedgerSeq: validUntil,
        delegates: [{ address: delegate.publicKey() }],
      });
      return authorizeEntry(withDelegates, delegate, validUntil, passphrase, delegate.publicKey());
    }),
  );

  const func = (first.operations[0] as { func: xdr.HostFunction }).func;
  const second = new TransactionBuilder(await server.getAccount(source.publicKey()), {
    fee: BASE_FEE,
    networkPassphrase: passphrase,
  })
    .addOperation(Operation.invokeHostFunction({ func, auth }))
    .setTimeout(120)
    .build();
  const enforced = await server.simulateTransaction(second);
  if (rpc.Api.isSimulationError(enforced)) {
    throw new SimulationFailed(method, enforced.error);
  }
  return submit(rpc.assembleTransaction(second, enforced).build(), source);
}

/** Like invokeDelegated, but must be rejected; returns the reason. */
export async function expectDelegatedRejected(
  source: Keypair,
  delegate: Keypair,
  accountId: string,
  contractId: string,
  method: string,
  args: xdr.ScVal[],
): Promise<string> {
  try {
    await invokeDelegated(source, delegate, accountId, contractId, method, args);
  } catch (e) {
    if (e instanceof SimulationFailed) return shortError(e.detail);
    throw e;
  }
  throw new Error(`${method} was expected to be rejected but succeeded`);
}

function shortError(detail: string): string {
  const m = detail.match(/Error\((\w+), ([#\w]+)\)/);
  return m ? `Error(${m[1]}, ${m[2]})` : detail.split("\n")[0].slice(0, 200);
}

// ---------------------------------------------------------------------------
// Evidence log
// ---------------------------------------------------------------------------

export type Step = {
  step: string;
  claim: string;
  hash?: string;
  explorer?: string;
  result?: string;
};

export class Evidence {
  readonly flow: string;
  readonly steps: Step[] = [];
  readonly context: Record<string, unknown> = {};

  constructor(flow: string) {
    this.flow = flow;
  }

  tx(step: string, claim: string, r: TxResult, result?: string) {
    const explorer = `https://stellar.expert/explorer/testnet/tx/${r.hash}`;
    this.steps.push({ step, claim, hash: r.hash, explorer, ...(result ? { result } : {}) });
    console.log(`  ✔ ${step}: ${claim}\n    ${explorer}${result ? `\n    → ${result}` : ""}`);
  }

  note(step: string, claim: string, result: string) {
    this.steps.push({ step, claim, result });
    console.log(`  ✔ ${step}: ${claim}\n    → ${result}`);
  }

  save() {
    const dir = join(ROOT, "deployments", "evidence");
    mkdirSync(dir, { recursive: true });
    const file = join(dir, `${this.flow}.json`);
    writeFileSync(
      file,
      JSON.stringify(
        { flow: this.flow, recordedAt: new Date().toISOString(), deployments, context: this.context, steps: this.steps },
        null,
        2,
      ) + "\n",
    );
    console.log(`\nSaved ${file}`);
  }
}

export const contractLink = (id: string) => `https://stellar.expert/explorer/testnet/contract/${id}`;
