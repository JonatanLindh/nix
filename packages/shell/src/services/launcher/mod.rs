//! What the launcher searches: `find` ranks apps and 1Password items, searches files and runs the
//! calculator; `clip` is clipboard history (cliphist); `passwords` is 1Password (the `op` CLI).
//! Once the qs-find, qs-clip and qs-op helpers in ~/nix.

pub mod clip;
pub mod find;
pub mod passwords;
