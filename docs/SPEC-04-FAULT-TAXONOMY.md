# SPEC-04: Fault Taxonomy & Fail-Stop Semantics

## 1. Operational Faults (Non-Fatal)
* P2P network disruptions, peer timeouts, malformed wire packets.
* Action: Terminate peer session; node continues normal operation.

## 2. Integrity Faults (Fatal Fail-Stop)
* Storage corruption, UTXO set inconsistency, invariant violation, disk commit failure.
* Action: Transition to `NodeState::Failed(NodeFault)`, halt all background workers, reject all further state mutations.
