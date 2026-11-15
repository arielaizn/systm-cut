//! Golden-image tests for SYSTM Cut's renderer (test-only crate; the library is empty).
//!
//! `tests/golden.rs` builds small procedural projects (no media files), renders one frame of each
//! through the CPU reference compositor (`systmcut-render`) and compares it with a committed PNG
//! in `goldens/` using `systmcut_testkit::golden`. The same scenes are composited on the GPU
//! (`systmcut-gpu`) when an adapter is available and compared with the CPU result.
//!
//! Regenerate references with `SYSTMCUT_BLESS=1 cargo test -p systmcut-golden`.
