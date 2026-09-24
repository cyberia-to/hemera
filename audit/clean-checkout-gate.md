# hemera clean-checkout gate

date: 2026-09-24
revision: 23f3bbcff910ea6d504ceb505680a539260869da (origin/main)
property: launch.md row 39 — every phase-1 component builds and tests from a
clean checkout of its default branch, no dead path, no version pin behind a
sibling, no crate present only in an owner's working tree.

hemera was not on the 2026-09-22/23 sweep's list of repos failing this gate
(bbg, cybergraph, foculus, mudra, neuron, nox, prysm, zheng, true-cyber,
glia, honeycrisp, rune, vault, radio), so this measures it directly rather
than assuming it is fine, the method file's row-39 audit and tru#24 used for
the same row.

hemera's four workspace members (`rs`, `wgsl`, `cli`, `bench`) depend on each
other only by in-workspace relative path (`../rs`); the workspace has no
path dependency on a sibling repository, so a clean checkout of hemera alone
is sufficient to exercise this gate — unlike file, which pulls hemera in as
`../hemera/rs`.

worktree: `git worktree add ... origin/main` (no local uncommitted state,
the owner's local checkout carries 13 unpushed commits ahead of this
revision and was not used).

```
$ RUSTC_BOOTSTRAP=1 cargo check --tests --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.69s

$ RUSTC_BOOTSTRAP=1 cargo test --workspace
test result: ok. 247 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 45 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

one warning (`unused import: super::*` in `rs/src/field.rs:322`), no errors.
result: hemera builds and tests clean from `origin/main`. this closes
hemera's own slice of row 39; the row stays open until every repo the sweep
found broken has its fix merged, and hemera's dependents (file, bbg,
cybergraph, foculus, tru, zheng, mudra) resolving it correctly is untested
here.
