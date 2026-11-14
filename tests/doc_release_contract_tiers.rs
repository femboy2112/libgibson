//! Every public module must have a stability tier in `docs/RELEASE_CONTRACT.md` §2.
//!
//! §2 sorts the public Rust surface into CORE, the utility tier (treated as CORE) and
//! EXPERIMENTAL. A module missing from §2 has no stated promise at all. Before v0.7.0, seventeen
//! modules had drifted out of it, `hold` and `text` among them. This guard is textual. It reads
//! every `pub mod name;` in `src/lib.rs` and requires `` `name` `` to appear in §2 (from the
//! `## 2.` heading up to `## 3.`). It does not check which tier is right; review decides that.

use std::path::Path;

fn read(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn public_modules(lib_rs: &str) -> Vec<String> {
    lib_rs
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub mod "))
        .filter_map(|rest| rest.strip_suffix(';'))
        .map(str::to_owned)
        .collect()
}

fn section_two(contract: &str) -> &str {
    let start = contract
        .find("\n## 2.")
        .expect("RELEASE_CONTRACT.md has a `## 2.` section");
    let end = contract[start + 1..]
        .find("\n## 3.")
        .map(|i| start + 1 + i)
        .expect("RELEASE_CONTRACT.md has a `## 3.` section after §2");
    &contract[start..end]
}

#[test]
fn every_public_module_has_a_tier_in_release_contract_section_2() {
    let modules = public_modules(&read("src/lib.rs"));
    assert!(
        modules.len() >= 40,
        "parser found only {} public modules: {modules:?}",
        modules.len()
    );
    let contract = read("docs/RELEASE_CONTRACT.md");
    let tiers = section_two(&contract);
    let missing: Vec<&String> = modules
        .iter()
        .filter(|m| !tiers.contains(&format!("`{m}`")))
        .collect();
    assert!(
        missing.is_empty(),
        "public modules with no tier in RELEASE_CONTRACT.md §2: {missing:?}"
    );
}

#[test]
fn the_guard_rejects_a_module_missing_from_section_2() {
    let lib = "pub mod alpha;\n#[cfg(unix)]\npub mod beta;\nmod hidden;\n";
    assert_eq!(public_modules(lib), ["alpha", "beta"]);
    let contract = "# T\n\n## 2. Tiers\n\n- `alpha`\n\n## 3. ABI\n\n- `beta`\n";
    let tiers = section_two(contract);
    assert!(tiers.contains("`alpha`"));
    assert!(!tiers.contains("`beta`"), "§3 text must not count for §2");
}
