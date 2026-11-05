//! The launcher's search, once the `qs-find` helper (helpers/ in ~/nix) and now called
//! directly: ranking apps and 1Password items with their launch counts ([`rank`]), the `=`
//! calculator through qalc ([`calc`], typeset with `tex`), and the file modes ([`files`]).
//!
//! Ranking is quick and runs where it's asked; calculations and file walks can take a while and
//! run on the services runtime, the launcher dropping the answers to queries it has moved on
//! from.

pub mod calc;
pub mod files;
pub mod rank;
mod tex;
