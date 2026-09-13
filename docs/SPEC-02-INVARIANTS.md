# SPEC-02: Ledger Invariants & Mathematical Tripwires

## 1. Konservasi Nilai
* Jumlah input kuanta eUTXO harus tepat sama atau lebih besar dari output + fee.
* Seluruh operasi kuanta menggunakan bilangan bulat `u128` murni (Zero-Float).

## 2. Imutabilitas eUTXO
* Input yang sudah dibelanjakan (*spent*) tidak dapat dibangkitkan kembali.
* Identitas outpoint bersifat deterministik dan unik.
