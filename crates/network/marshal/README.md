# `kora-marshal`

The marshal handles block dissemination through commonware's
[`Marshaled`][marshaled] application adapter.


The [`Marshaled`][marshaled] adapter handles epoch transitions
and validates block ancestry. This adapter "wraps" the application
to handle the following ancestry checks automatically during
verification:

- **Block Ancestry**: Parent Commitment matches the consensus context's expected parent.
- **Epoch Transitions**: Block height is exactly one greater than the parent's height.

[marshaled]: https://docs.rs/commonware-consensus/latest/commonware_consensus/application/marshaled/struct.Marshaled.html
