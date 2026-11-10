// stellar-8004 agent wallet flow on Stellar testnet.
//
// Gives a stellar-8004 agent NFT its own Heybe account, binds that account as
// the agent's wallet, transfers the agent, and shows that the new owner
// controls the same funds. Nothing in stellar-8004 is changed: Heybe reads the
// identity registry's standard SEP-50 owner_of.
//
//   cd scripts && npm run flow:8004

import {
  Evidence,
  addr,
  contractLink,
  deployments as d,
  expectFailedOnChain,
  i128,
  identity,
  invoke,
  read,
  str,
  sym,
  u32,
  vec,
  xlm,
} from "../lib/heybe.ts";

const ali = identity("heybe-ali");
const ayse = identity("heybe-ayse");
const bob = identity("heybe-bob");
const registry8004 = d.stellar8004IdentityRegistry;
const ev = new Evidence("stellar-8004");

const pay = (from: string, to: string, amount: number) =>
  [addr(d.xlmSac), sym("transfer"), vec([addr(from), addr(to), i128(xlm(amount))])];
const bindWallet = (account: string, caller: string, agentId: number) =>
  [addr(registry8004), sym("set_agent_wallet"), vec([addr(caller), u32(agentId), addr(account)])];

console.log("Heybe × stellar-8004 agent wallet flow — Stellar testnet\n");

// 1. Register an agent (agent NFT minted to Ali)
const reg = await invoke(ali, registry8004, "register_with_uri", [
  addr(ali.publicKey()),
  str("https://github.com/heybeonstellar/heybe#test-agent"),
]);
const agentId = Number(reg.value);
ev.tx("register_agent", `Ali registers a test agent on stellar-8004 and receives agent NFT #${agentId}`, reg);

// 2. Create the agent's Heybe account
const created = await invoke(ali, d.registry, "create_account", [addr(registry8004), u32(agentId)]);
const account = String(created.value);
ev.tx("create_account", "Ali creates the Heybe account for the agent NFT", created);
Object.assign(ev.context, {
  agentId,
  account,
  accountExplorer: contractLink(account),
  identityRegistry: registry8004,
});

// 3. Fund it
const fund = await invoke(ali, d.xlmSac, "transfer", [addr(ali.publicKey()), addr(account), i128(xlm(10))]);
ev.tx("fund_account", "Ali funds the agent's account with 10 XLM", fund);

// 4. Bind it as the agent wallet (Path A; the account is new_wallet and the direct invoker)
const bind = await invoke(ali, account, "execute", bindWallet(account, ali.publicKey(), agentId));
ev.tx("bind_agent_wallet", "Through execute, Ali calls set_agent_wallet(Ali, agent, account). The account authorizes as new_wallet", bind);
const wallet = await read(registry8004, "get_agent_wallet", [u32(agentId)], ali);
ev.note("agent_wallet_is_account", "stellar-8004 now reports the Heybe account as the agent wallet", String(wallet));

// 5. Transfer the agent NFT to Ayşe
const move = await invoke(ali, registry8004, "transfer", [addr(ali.publicKey()), addr(ayse.publicKey()), u32(agentId)]);
ev.tx("transfer_agent", "Ali transfers the agent NFT to Ayşe", move);
const walletAfter = await read(registry8004, "get_agent_wallet", [u32(agentId)], ayse);
ev.note(
  "registry_clears_wallet",
  "As designed, stellar-8004 clears agentWallet on transfer",
  walletAfter === undefined || walletAfter === null ? "none" : String(walletAfter),
);
const owner = String(await read(account, "owner", [], ayse));
ev.note("account_follows_agent", "The Heybe account now reports Ayşe as owner. Its funds stayed with the agent", owner);

// 6. Ayşe spends the agent's funds
const spend = await invoke(ayse, account, "execute", pay(account, bob.publicKey(), 3));
ev.tx("new_owner_spends", "Ayşe pays 3 XLM from the agent's account", spend);

// 7. Ayşe re-binds the same account as the agent wallet
const rebind = await invoke(ayse, account, "execute", bindWallet(account, ayse.publicKey(), agentId));
ev.tx("rebind_agent_wallet", "Ayşe re-binds the same account as the agent wallet in one call", rebind);
const walletRebound = await read(registry8004, "get_agent_wallet", [u32(agentId)], ayse);
ev.note("agent_wallet_rebound", "stellar-8004 reports the same Heybe account again", String(walletRebound));

// 8. Ali can no longer touch the agent's funds
const old = await expectFailedOnChain(ali, account, "execute", pay(account, ali.publicKey(), 5));
ev.tx(
  "previous_owner_rejected",
  "Ali signs execute on the agent's account. The transaction fails on chain",
  old,
  `failed as expected; required signer: ${old.requiredSigner}`,
);

const balance = await read(d.xlmSac, "balance", [addr(account)], ayse);
ev.note("final_balance", "Agent account balance (10 - 3)", `${Number(balance) / 1e7} XLM`);

ev.save();
