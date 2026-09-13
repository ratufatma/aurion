# SPEC-03: Storage & Persistence Protocol

## 1. Exclusive redb Authority
* The underlying `redb` storage engine must only be opened and managed by `aurion-node`.
* Disk mutations must be strictly atomic (`WriteTransaction`).
* Any disk commit failure on canonical blocks constitutes a fatal *Integrity Fault*, immediately triggering `NodeState::Failed`.
