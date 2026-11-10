# Demo (planned)

The testnet demo shows one claim: **control of the account follows the NFT.** Every step links to its transaction on [stellar.expert](https://stellar.expert/explorer/testnet).

## Web demo

Requires the Freighter browser extension on testnet. You need two accounts, Ali and Ayşe.

| Step | What you do | What you see |
|---|---|---|
| 1 | Connect Freighter as Ali | Ali's testnet address and XLM balance |
| 2 | Mint a test NFT (or enter a stellar-8004 agent id) | Token id and the NFT contract address |
| 3 | Look up the account | The Heybe account address, computed before it exists |
| 4 | Create the account | Deployment transaction |
| 5 | Send 100 XLM to the account | The account's balance |
| 6 | Pay 10 XLM from the account to any address (Path A) | `execute` transaction; balance drops to 90 |
| 7 | Transfer the NFT to Ayşe | NFT transfer transaction; the account shows Ayşe as owner |
| 8 | Ali tries to pay from the account again | Rejected in simulation: Ali is no longer the owner |
| 9 | Switch Freighter to Ayşe and pay from the account | Succeeds; same account address, new controller |

## stellar-8004 flow (script, then UI if time allows)

Uses the stellar-8004 testnet Identity Registry `CDE3K4COIAGWNNJQQLL26SYI3KBJF5FUDHXG5FA6GYDJCG7T5V7FIWZH`.

1. Ali registers an agent and receives agent NFT `#n`.
2. Ali creates the Heybe account for `(identity_registry, n)`.
3. Ali calls `execute(identity_registry, "set_agent_wallet", [Ali, n, account])`.
4. `get_agent_wallet(n)` now returns the Heybe account.
5. Ali transfers the agent NFT to Ayşe. The registry clears `agentWallet` on transfer, as designed.
6. Ayşe controls the same Heybe account, funds included. She can re-bind it with `set_agent_wallet` in one call.

If the stellar-8004 testnet deployment is unavailable, the scripts deploy the open-source identity registry wasm on testnet and run the same flow.

## Evidence

Every flow above will record its transaction hashes in `EVIDENCE.md` at the repository root once implemented.
