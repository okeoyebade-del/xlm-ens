# Testnet Operator Runbook (Deploy → Register → Resolve)

This runbook is focused on an operator or integrator running the end-to-end flow on **Stellar testnet**:

1. Configure RPC + network passphrase.
2. Deploy contracts (or point at existing deployments).
3. Register a name.
4. Resolve it forward and reverse.
5. Set text records.

## Prerequisites

- `scripts/bootstrap.sh --install` (or ensure tooling is installed)
- A funded testnet account for any write operations (registration, transfers, text-record writes).

## Environment

The CLI reads configuration from env vars (with testnet defaults):

- `SOROBAN_RPC_URL` (default: `https://soroban-testnet.stellar.org`)
- `SOROBAN_NETWORK_PASSPHRASE` (default: `Test SDF Network ; September 2015`)
- `REGISTRY_CONTRACT_ID` (required unless you are using a build that bakes defaults)
- `RESOLVER_CONTRACT_ID` (required unless you are using a build that bakes defaults)

For signing, profiles are loaded from env vars:

- `XLM_NS_SIGNER_<PROFILE>_PUBLIC`
- `XLM_NS_SIGNER_<PROFILE>_SECRET`

Example (replace values):

```sh
export XLM_NS_SIGNER_OPERATOR_PUBLIC="G..."
export XLM_NS_SIGNER_OPERATOR_SECRET="S..."
export REGISTRY_CONTRACT_ID="C..."
export RESOLVER_CONTRACT_ID="C..."
```

## Deploy (testnet)

If you already have contract IDs for testnet, you can skip deployment and set
`REGISTRY_CONTRACT_ID` / `RESOLVER_CONTRACT_ID`.

If you are deploying yourself, use the Soroban CLI for your environment and
record the resulting contract IDs.

## Register a name

Build the CLI:

```sh
cargo build -p xlm-ns-cli
```

Register a name (example):

```sh
cargo run -p xlm-ns-cli -- register timmy.xlm "$XLM_NS_SIGNER_OPERATOR_PUBLIC" --signer operator
```

## Resolve a name

Forward resolution:

```sh
cargo run -p xlm-ns-cli -- resolve timmy.xlm
```

Reverse lookup:

```sh
cargo run -p xlm-ns-cli -- reverse-lookup "$XLM_NS_SIGNER_OPERATOR_PUBLIC"
```

## Portfolio

List names owned by an address:

```sh
cargo run -p xlm-ns-cli -- portfolio "$XLM_NS_SIGNER_OPERATOR_PUBLIC"
```

### Export formats

The `portfolio` command supports three output modes:

```sh
# Human-readable table (default)
cargo run -p xlm-ns-cli -- portfolio OWNER_ADDRESS

# JSON array — suitable for jq, APIs, or storage
cargo run -p xlm-ns-cli -- portfolio OWNER_ADDRESS --output json

# CSV — suitable for spreadsheets or data pipelines
cargo run -p xlm-ns-cli -- portfolio OWNER_ADDRESS --output csv \
  > owner-export.csv

# Large portfolios are fetched in pages of 50 by default. Tune the page size,
# cap the export, or request one 1-based page when operating on registrar/DAO wallets.
cargo run -p xlm-ns-cli -- portfolio OWNER_ADDRESS --batch-size 25 --limit 100
cargo run -p xlm-ns-cli -- portfolio OWNER_ADDRESS --page 2 --output json
```

Each record includes: `name`, `owner`, `resolver`,
`target_address`, `registered_at`, `expires_at`,
`grace_period_ends_at`, and `status`
(`active` | `grace` | `expired`).

## Text records

Set a text record:

```sh
cargo run -p xlm-ns-cli -- text set timmy.xlm url "https://example.com" --signer operator
```

Read a text record:

```sh
cargo run -p xlm-ns-cli -- text get timmy.xlm url
```

