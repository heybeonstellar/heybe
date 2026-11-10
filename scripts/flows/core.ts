// Core Heybe flow on Stellar testnet.
//
// Proves, with public transactions, that control of a Heybe account follows
// its NFT: funding before deployment, permissionless creation, Path A
// (execute), Path B (CAP-71 delegation with a real G-key signature), the
// self-transfer guard and the lock.
//
//   cd scripts && npm run flows
//
// Uses stellar CLI identities heybe-ali, heybe-ayse, heybe-bob (funded on testnet).

import {
  Evidence,
  addr,
  contractLink,
  deployments as d,
  expectDelegatedRejected,
  expectFailedOnChain,
  expectRejected,
  i128,
  identity,
  invoke,
  invokeDelegated,
  read,
  server,
  sym,
  u32,
  vec,
  xlm,
} from "../lib/heybe.ts";

const ali = identity("heybe-ali");
const ayse = identity("heybe-ayse");
const bob = identity("heybe-bob");
const ev = new Evidence("core");

const pay = (from: string, to: string, amount: number) =>
  [addr(d.xlmSac), sym("transfer"), vec([addr(from), addr(to), i128(xlm(amount))])];

console.log("Heybe core flow — Stellar testnet\n");

// 1. Mint
const mint = await invoke(ali, d.testNft, "mint", [addr(ali.publicKey())]);
const tokenId = Number(mint.value);
ev.tx("mint", `Ali mints test NFT #${tokenId}`, mint);

// 2. Address before deployment
const accountId = String(await read(d.registry, "account_of", [addr(d.testNft), u32(tokenId)], ali));
ev.note("account_of", "The registry returns the account address before the account exists", accountId);
Object.assign(ev.context, {
  tokenId,
  account: accountId,
  accountExplorer: contractLink(accountId),
  ali: ali.publicKey(),
  ayse: ayse.publicKey(),
  bob: bob.publicKey(),
});

// 3. Fund before deployment
const pre = await invoke(ali, d.xlmSac, "transfer", [addr(ali.publicKey()), addr(accountId), i128(xlm(20))]);
ev.tx("fund_before_deploy", "Ali sends 20 XLM to the account address before the account contract exists", pre);

// 4. Permissionless creation
const created = await invoke(bob, d.registry, "create_account", [addr(d.testNft), u32(tokenId)]);
if (String(created.value) !== accountId) throw new Error("create_account returned a different address");
ev.tx("create_account", "Bob, who does not own the NFT, deploys the account. Creating grants no control", created);

const bal = await read(d.xlmSac, "balance", [addr(accountId)], ali);
ev.note("balance_after_deploy", "The 20 XLM sent before deployment are in the deployed account", `${Number(bal) / 1e7} XLM`);

// 5. Path A: owner pays
const payAli = await invoke(ali, accountId, "execute", pay(accountId, bob.publicKey(), 5));
ev.tx("path_a_owner_pays", "Ali (NFT owner) pays 5 XLM from the account with execute", payAli);

// 6. Path A: stranger rejected on chain
const stranger = await expectFailedOnChain(bob, accountId, "execute", pay(accountId, bob.publicKey(), 15));
ev.tx(
  "path_a_stranger_rejected",
  "Bob signs and submits execute himself. The transaction fails on chain: the account requires the owner's signature",
  stranger,
  `failed as expected; required signer: ${stranger.requiredSigner}`,
);

// 7. NFT moves to Ayşe
const move = await invoke(ali, d.testNft, "transfer", [addr(ali.publicKey()), addr(ayse.publicKey()), u32(tokenId)]);
ev.tx("nft_transfer", "Ali transfers the NFT to Ayşe. No Heybe transaction is involved", move);
const ownerNow = String(await read(accountId, "owner", [], ali));
ev.note("owner_after_transfer", "The account now reports Ayşe as its owner", ownerNow);

// 8. Old owner rejected on chain
const old = await expectFailedOnChain(ali, accountId, "execute", pay(accountId, ali.publicKey(), 5));
ev.tx(
  "path_a_old_owner_rejected",
  "Ali, the previous owner, signs execute. The transaction fails on chain: the account now requires Ayşe",
  old,
  `failed as expected; required signer: ${old.requiredSigner}`,
);

// 9. New owner pays
const payAyse = await invoke(ayse, accountId, "execute", pay(accountId, bob.publicKey(), 5));
ev.tx("path_a_new_owner_pays", "Ayşe (new owner) pays 5 XLM from the same account address", payAyse);

// 10. Path B: CAP-71 delegated auth with a real G-key signature
const pathB = await invokeDelegated(bob, ayse, accountId, d.xlmSac, "transfer", [
  addr(accountId),
  addr(bob.publicKey()),
  i128(xlm(2)),
]);
ev.tx(
  "path_b_delegated",
  "CAP-71: xlm.transfer(from = account) is authorized by the account's __check_auth. Ayşe's G key signs as the delegate; Bob only submits",
  pathB,
);

// 11. Path B with the previous owner as delegate
const pathBOld = await expectDelegatedRejected(bob, ali, accountId, d.xlmSac, "transfer", [
  addr(accountId),
  addr(bob.publicKey()),
  i128(xlm(2)),
]);
ev.note("path_b_old_owner_rejected", "CAP-71 with Ali (previous owner) as the delegate is rejected", pathBOld);

// 12. Self-transfer guard
const selfTransfer = await expectRejected(ayse, accountId, "execute", [
  addr(d.testNft),
  sym("transfer"),
  vec([addr(ayse.publicKey()), addr(accountId), u32(tokenId)]),
]);
ev.note("guard_self_transfer", "Sending the bound NFT into its own account through execute is blocked (SelfTransferBlocked = #3)", selfTransfer);

// 13. Lock
const latest = await server.getLatestLedger();
const until = latest.sequence + 24;
const lock = await invoke(ayse, accountId, "lock", [u32(until)]);
ev.tx("lock", `Ayşe locks the account until ledger ${until} (about 2 minutes), as a seller would during a sale`, lock);
const locked = await expectRejected(ayse, accountId, "execute", pay(accountId, ayse.publicKey(), 1));
ev.note("locked_rejected", "While locked, even the owner cannot spend (Locked = #1)", locked);

// 14. State counter
const state = await read(accountId, "state", [], ayse);
ev.note("state_counter", "state() counts every execute, successful __check_auth and lock", String(state));
const finalBalance = await read(d.xlmSac, "balance", [addr(accountId)], ayse);
ev.note("final_balance", "Account balance after the flow (20 - 5 - 5 - 2)", `${Number(finalBalance) / 1e7} XLM`);

ev.save();
