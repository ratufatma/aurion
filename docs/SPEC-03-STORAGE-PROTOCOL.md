# SPEC-03: Storage & Persistence Protocol

## 1. Otoritas Tunggal redb
* Penyimpanan `redb` hanya boleh dibuka oleh `aurion-node`.
* Mutasi disk wajib bersifat atomik (WriteTransaction).
* Kegagalan komit disk pada blok kanonikal adalah *Integrity Fault* fatal yang langsung memicu status `NodeState::Failed`.
