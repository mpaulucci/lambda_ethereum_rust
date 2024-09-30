# Ethereum Rust L2 Contracts

## ToC

- [L1 side](#l1-side)
    - [`CommonBridge`](#commonbridge)
    - [`BlockExecutor`](#blockexecutor)
- [L2 side](#l2-side)
    - [`L1MessageSender`](#l1messagesender)

## L1 side

### `CommonBridge`

Allows L1<->L2 communication from L1. It both sends messages from L1 to L2 and receives messages from L2.

### `BlockExecutor`

Ensures the advancement of the L2. It is used by the operator to commit blocks and verify block proofs

### `Verifier`

TODO

## L2 side

### `L1MessageSender`

TODO
