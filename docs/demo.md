# Demo

The demo shows one claim: **control of the account follows the NFT.** Every step links to its transaction on [stellar.expert](https://stellar.expert/explorer/testnet).

## Scripted flows (done)

Two scripts run the whole lifecycle on testnet and record every transaction in [EVIDENCE.md](../EVIDENCE.md).

**Core flow:** `cd scripts && npm run flows`

1. Ali mints a test NFT.
2. The registry gives the account address before the account exists.
3. Ali funds that address.
4. Bob, who does not own the NFT, deploys the account.
5. Ali pays from the account (Path A).
6. Bob tries the same and fails on chain.
7. The NFT moves to Ayşe.
8. Ali is now rejected on chain, and Ayşe pays from the same account.
9. Ayşe authorizes a direct XLM transfer from the account through CAP-71 (Path B). Her G key is the delegate, and Bob only submits.
10. The self-transfer guard and the lock reject what they should.

**stellar-8004 flow:** `cd scripts && npm run flow:8004`

It uses the stellar-8004 testnet identity registry `CDE3K4COIAGWNNJQQLL26SYI3KBJF5FUDHXG5FA6GYDJCG7T5V7FIWZH` and does not change it.

1. Ali registers a test agent and receives agent NFT `#n`.
2. Ali creates the Heybe account for `(identity_registry, n)` and funds it.
3. Ali calls `execute(identity_registry, "set_agent_wallet", [Ali, n, account])`. The account authorizes as `new_wallet`, because it is the direct invoker.
4. `get_agent_wallet(n)` returns the Heybe account.
5. Ali transfers the agent NFT to Ayşe. The registry clears `agentWallet`, as designed.
6. The account now answers to Ayşe. She spends from it and re-binds it as the agent wallet in one call.
7. Ali's attempt to spend fails on chain.

## Web demo (next)

Requires the Freighter browser extension on testnet and two accounts, Ali and Ayşe.

| Step | What you do | What you see |
|---|---|---|
| 1 | Connect Freighter as Ali | Ali's testnet address and XLM balance |
| 2 | Mint a test NFT, or enter a stellar-8004 agent id | Token id and the NFT contract address |
| 3 | Look up the account | The Heybe account address, computed before it exists |
| 4 | Create the account | Deployment transaction |
| 5 | Send XLM to the account | The account's balance |
| 6 | Pay from the account to any address (Path A) | `execute` transaction and the new balance |
| 7 | Transfer the NFT to Ayşe | NFT transfer transaction; the account shows Ayşe as owner |
| 8 | Ali tries to pay from the account again | Rejected: Ali is no longer the owner |
| 9 | Switch Freighter to Ayşe and pay from the account | Succeeds: same account address, new controller |
