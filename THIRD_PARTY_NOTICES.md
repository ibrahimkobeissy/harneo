# Third-party notices

## HarnessKit

Harneo's security audit rules are ported from, and adapted from, HarnessKit.

- Project: HarnessKit, https://github.com/RealZST/HarnessKit
- Commit: `9098728d22fd5c2ea6fb4864da19183b55cb33c4`
- License: Apache License 2.0, in full at [`licenses/HarnessKit-APACHE-2.0.txt`](licenses/HarnessKit-APACHE-2.0.txt)
- Notice: HarnessKit ships no NOTICE file.
- Source files used: `crates/hk-core/src/auditor/` (`mod.rs`, `rules.rs`, `rules/*.rs`) and `crates/hk-core/src/scanner.rs` (reach inference).

Every Harneo file that contains ported or adapted code starts with a header naming the HarnessKit file, the commit, the license, and what was modified.
