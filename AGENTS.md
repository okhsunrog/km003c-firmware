# Development

- Use Rust edition 2024 and cargo add/remove for dependency changes.
- Use uv run for Python. Reuse the sibling km003c protocol implementation;
  do not duplicate authenticated framing or derive keys in new scripts.
- Preserve the native bootloader and ICG. Applications begin at 0x4000.
- Verify backup, recovery entry, image readback and unchanged bootloader before
  claiming hardware success. Keep build checks distinct from device tests.
- Temporary diagnostic artifacts belong under
  /home/okhsunrog/tmp_zfs/codex_tmp/km003c-rust-bringup/.
- Do not add assistant attribution to commits or PRs.
- Generic HC32 HAL changes belong in the user's fork:
  <https://github.com/okhsunrog/hc32f460-embassy-rs>
  (upstream <https://github.com/kralonur/hc32f460-embassy-rs>).
  The user authorizes pushing necessary HAL changes to their fork.
  Keep KM003C board configuration and application policy in this repository.
- During repeated hardware tests, keep the working custom application installed.
  Do not automatically restore stock after each run. Install stock only for a
  concrete reference comparison, recovery, or an explicit user request. Preserve
  the verified rollback package; verify a flash write when one is necessary.
  Group code changes into useful hardware runs to avoid needless erase cycles.
