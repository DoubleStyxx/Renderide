//! Golden-case coverage for every material root the renderer ships.
//!
//! The authoritative root list is [`crate::embedded_shaders::COMPILED_MATERIAL_STEMS`],
//! the build-script registry. `stem_metadata` queries stems; it does not list them.
//! A root strips `_default` / `_multiview`. `shaders/materials/*.wgsl` file stems are
//! a subset: a `//#keyword` fan-out such as `pbsmetallic_alphapremultiply` is an extra
//! shipped root.
//!
//! Suite cases come from [`renderide_test::scene_dsl::material_usage::golden_case_materials`],
//! which reads the shader each case uploads. [`MAX_ALLOWLIST_ROOTS`] is the growth
//! guard for [`ALLOWLIST_FILE`]. The only allowed edit to that constant is lowering
//! it after an allowlist entry is deleted.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::materials::custom_shader::normalize_shader_key;

/// Allowlist of material roots that have no golden case yet.
///
/// Every entry needs a non-empty `reason` or an `issue` link (`http://` or `https://`).
/// An empty `reason` is rejected. The file may only shrink: a root that gains a case
/// must be removed, and a root that is not shipped must not be named.
const ALLOWLIST_FILE: &str = "material_coverage_allowlist.toml";

/// Upper bound on [`ALLOWLIST_FILE`].
///
/// The allowlist may only shrink. The only allowed edit to this constant is lowering
/// it in the same change that deletes an entry. Raising it hides a new uncovered root.
const MAX_ALLOWLIST_ROOTS: usize = 145;

const DOC_RELATIVE: &str = "../../docs/testing/material-coverage.md";

/// `UPDATE_MATERIAL_COVERAGE=1 cargo test -p renderide --lib material_coverage::tests::material_coverage_doc_matches_sources -- --exact`
const REGENERATE_DOC: &str = "UPDATE_MATERIAL_COVERAGE=1 cargo test -p renderide --lib material_coverage::tests::material_coverage_doc_matches_sources -- --exact";

#[derive(Clone, Debug, Eq, PartialEq)]
struct AllowEntry {
    root: String,
    justification: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoverageIndex {
    cases_by_root: BTreeMap<String, BTreeSet<String>>,
    cases_without_shipped_root: BTreeSet<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AllowlistFile {
    #[serde(default)]
    entry: Vec<AllowlistRaw>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AllowlistRaw {
    root: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    issue: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum BackfillBucket {
    Pbs,
    Msaa,
    HdrTonemap,
    Skinning,
    Text,
    Particles,
    Remaining,
}

impl BackfillBucket {
    const fn label(self) -> &'static str {
        match self {
            Self::Pbs => "PBS variants (metallic, specular, premultiply, and keyword variants)",
            Self::Msaa => "MSAA",
            Self::HdrTonemap => "HDR/tonemap",
            Self::Skinning => "skinning",
            Self::Text => "text",
            Self::Particles => "particles",
            Self::Remaining => "remaining",
        }
    }

    fn all() -> &'static [Self] {
        &[
            Self::Pbs,
            Self::Msaa,
            Self::HdrTonemap,
            Self::Skinning,
            Self::Text,
            Self::Particles,
            Self::Remaining,
        ]
    }
}

fn allowlist_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(ALLOWLIST_FILE)
}

fn doc_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DOC_RELATIVE)
}

fn shipped_material_roots() -> BTreeSet<String> {
    crate::embedded_shaders::COMPILED_MATERIAL_STEMS
        .iter()
        .map(|stem| material_root_from_compiled_stem(stem).to_string())
        .collect()
}

fn material_root_from_compiled_stem(stem: &str) -> &str {
    stem.strip_suffix("_default")
        .or_else(|| stem.strip_suffix("_multiview"))
        .unwrap_or(stem)
}

fn material_shader_file_stems() -> BTreeSet<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("shaders/materials");
    let mut stems = BTreeSet::new();
    let entries = fs::read_dir(&dir).unwrap_or_else(|err| panic!("read {}: {err}", dir.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|err| panic!("read {}: {err}", dir.display()));
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("wgsl") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_else(|| panic!("non-utf8 shader stem {}", path.display()));
        assert!(
            stems.insert(stem.to_string()),
            "duplicate shader stem {stem}"
        );
    }
    stems
}

fn load_allowlist(path: &Path) -> Result<Vec<AllowEntry>, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    parse_allowlist(&text)
}

fn parse_allowlist(text: &str) -> Result<Vec<AllowEntry>, String> {
    let file: AllowlistFile =
        toml::from_str(text).map_err(|err| format!("parse material coverage allowlist: {err}"))?;
    let mut entries = Vec::with_capacity(file.entry.len());
    let mut seen = BTreeSet::new();
    let mut errors = Vec::new();
    for raw in file.entry {
        match allow_entry(&raw) {
            Ok(entry) => {
                if !seen.insert(entry.root.clone()) {
                    errors.push(format!("allowlist entry `{}` is duplicated", entry.root));
                }
                entries.push(entry);
            }
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty() {
        Ok(entries)
    } else {
        Err(errors.join("\n"))
    }
}

fn allow_entry(raw: &AllowlistRaw) -> Result<AllowEntry, String> {
    let root = raw.root.trim();
    if root.is_empty() {
        return Err("allowlist entry has an empty root".to_string());
    }
    if raw
        .reason
        .as_ref()
        .is_some_and(|reason| reason.trim().is_empty())
    {
        return Err(format!("allowlist entry `{root}` has an empty reason"));
    }
    if raw
        .issue
        .as_ref()
        .is_some_and(|issue| issue.trim().is_empty())
    {
        return Err(format!("allowlist entry `{root}` has an empty issue link"));
    }
    let reason = trimmed_nonempty(raw.reason.as_deref());
    let issue = trimmed_nonempty(raw.issue.as_deref());
    if let Some(issue) = issue
        && !(issue.starts_with("https://") || issue.starts_with("http://"))
    {
        return Err(format!(
            "allowlist entry `{root}` issue is not a link: {issue}"
        ));
    }
    let justification = match (reason, issue) {
        (Some(reason), Some(issue)) => format!("{reason} ({issue})"),
        (Some(reason), None) => reason.to_string(),
        (None, Some(issue)) => issue.to_string(),
        (None, None) => {
            return Err(format!(
                "allowlist entry `{root}` needs a reason or an issue link"
            ));
        }
    };
    Ok(AllowEntry {
        root: root.to_string(),
        justification,
    })
}

fn trimmed_nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty())
}

fn index_draws(
    roots: &BTreeSet<String>,
    draws: &[renderide_test::scene_dsl::material_usage::GoldenCaseMaterials],
) -> Result<CoverageIndex, String> {
    let mut errors = Vec::new();
    let mut cases_by_root: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut cases_without_shipped_root = BTreeSet::new();
    for draw in draws {
        let mut covered = false;
        if draw.null_fallback {
            covered = true;
            record_case(
                roots,
                &mut cases_by_root,
                "null",
                &draw.case,
                "null fallback",
                &mut errors,
            );
        }
        for shader in &draw.shaders {
            covered = true;
            let root = root_for_draw(shader.shader_asset, shader.variant_bits);
            record_case(
                roots,
                &mut cases_by_root,
                &root,
                &draw.case,
                shader.shader_asset,
                &mut errors,
            );
        }
        if !covered {
            cases_without_shipped_root.insert(draw.case.clone());
        }
    }
    if errors.is_empty() {
        Ok(CoverageIndex {
            cases_by_root,
            cases_without_shipped_root,
        })
    } else {
        Err(errors.join("\n"))
    }
}

fn root_for_draw(shader: &str, variant_bits: Option<u32>) -> String {
    let base = normalize_shader_key(shader);
    let selected = crate::materials::embedded::stem_metadata::pbsmetallic_compile_time_stem(
        format!("{base}_default"),
        variant_bits.unwrap_or(0),
    );
    material_root_from_compiled_stem(&selected).to_string()
}

fn record_case(
    roots: &BTreeSet<String>,
    cases_by_root: &mut BTreeMap<String, BTreeSet<String>>,
    root: &str,
    case: &str,
    shader: &str,
    errors: &mut Vec<String>,
) {
    if !roots.contains(root) {
        errors.push(format!(
            "golden case `{case}` draws `{shader}` (root `{root}`), which is not a shipped material root"
        ));
        return;
    }
    cases_by_root
        .entry(root.to_string())
        .or_default()
        .insert(case.to_string());
}

fn check_coverage(
    roots: &BTreeSet<String>,
    index: &CoverageIndex,
    allowlist: &[AllowEntry],
    max_allowlist: usize,
) -> Result<(), String> {
    let mut errors = Vec::new();
    let allow_roots: BTreeSet<&str> = allowlist.iter().map(|entry| entry.root.as_str()).collect();
    for root in roots {
        let covered = index
            .cases_by_root
            .get(root)
            .is_some_and(|cases| !cases.is_empty());
        if !covered && !allow_roots.contains(root.as_str()) {
            errors.push(format!(
                "uncovered material root `{root}` has no golden case and is not allowlisted"
            ));
        }
    }
    for entry in allowlist {
        if !roots.contains(&entry.root) {
            errors.push(format!(
                "allowlist entry `{}` is not a shipped material root",
                entry.root
            ));
            continue;
        }
        if let Some(cases) = index.cases_by_root.get(&entry.root) {
            let listed = cases.iter().cloned().collect::<Vec<_>>().join(", ");
            errors.push(format!(
                "stale allowlist entry `{}` has golden cases: {listed}",
                entry.root
            ));
        }
    }
    if allowlist.len() > max_allowlist {
        errors.push(format!(
            "allowlist has {} entries; MAX_ALLOWLIST_ROOTS is {max_allowlist}; the only allowed edit to that constant is lowering it",
            allowlist.len()
        ));
    }
    errors.sort();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

fn backfill_bucket(root: &str) -> BackfillBucket {
    if root.starts_with("pbs") || root == "paintpbs" {
        return BackfillBucket::Pbs;
    }
    if root.contains("a2c") || root.contains("msaa") {
        return BackfillBucket::Msaa;
    }
    if root.starts_with("gamma")
        || root == "lut"
        || root.starts_with("lut_")
        || root.contains("tonemap")
        || root.contains("hdr")
    {
        return BackfillBucket::HdrTonemap;
    }
    if root.contains("skin") {
        return BackfillBucket::Skinning;
    }
    if root == "textunlit" || root == "ui_textunlit" {
        return BackfillBucket::Text;
    }
    if root.contains("billboard") || root.contains("particle") {
        return BackfillBucket::Particles;
    }
    BackfillBucket::Remaining
}

fn uncovered_roots<'a>(roots: &'a BTreeSet<String>, index: &'a CoverageIndex) -> Vec<&'a str> {
    roots
        .iter()
        .filter(|root| !index.cases_by_root.contains_key(root.as_str()))
        .map(String::as_str)
        .collect()
}

fn render_doc(roots: &BTreeSet<String>, index: &CoverageIndex, allowlist: &[AllowEntry]) -> String {
    let mut reasons = BTreeMap::new();
    for entry in allowlist {
        reasons.insert(entry.root.as_str(), entry.justification.as_str());
    }
    let covered = roots
        .iter()
        .filter(|root| index.cases_by_root.contains_key(root.as_str()))
        .count();
    let mut doc = String::new();
    doc.push_str("# Material coverage\n\n");
    doc.push_str("Generated from `COMPILED_MATERIAL_STEMS` and the scene-DSL golden cases.\n");
    doc.push_str("Do not edit this file by hand.\n\n");
    doc.push_str("Regenerate it with:\n\n```\n");
    doc.push_str(REGENERATE_DOC);
    doc.push_str("\n```\n\n");
    write!(
        doc,
        "Shipped roots: {}. Covered: {covered}. Allowlisted: {}.\n\n",
        roots.len(),
        allowlist.len()
    )
    .expect("format coverage summary");
    doc.push_str(&render_custom_cases(index));
    doc.push('\n');
    doc.push_str(&render_table(roots, index, &reasons));
    doc.push('\n');
    doc.push_str(&render_buckets(roots, index));
    doc
}

fn render_custom_cases(index: &CoverageIndex) -> String {
    let mut section =
        String::from("Suite cases that draw a custom shader, not a shipped material root:\n\n");
    if index.cases_without_shipped_root.is_empty() {
        section.push_str("- (none)\n");
        return section;
    }
    for case in &index.cases_without_shipped_root {
        writeln!(section, "- `{case}`").expect("format custom-shader case");
    }
    section
}

fn render_table(
    roots: &BTreeSet<String>,
    index: &CoverageIndex,
    reasons: &BTreeMap<&str, &str>,
) -> String {
    let mut table = String::from("| Root | Golden cases | Allowlist |\n| --- | --- | --- |\n");
    for root in roots {
        let cases = index
            .cases_by_root
            .get(root)
            .map(|names| {
                names
                    .iter()
                    .map(|name| format!("`{name}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let reason = reasons
            .get(root.as_str())
            .copied()
            .unwrap_or("")
            .replace('|', "\\|")
            .replace('\n', " ");
        writeln!(table, "| `{root}` | {cases} | {reason} |").expect("format coverage row");
    }
    table
}

fn render_buckets(roots: &BTreeSet<String>, index: &CoverageIndex) -> String {
    let uncovered = uncovered_roots(roots, index);
    let mut section = String::from(
        "## Uncovered roots by backfill bucket\n\nPremultiply is a keyword inside PBS stems, not its own root. No shipped stem is named tonemap or skinning; those buckets list the roots the classifier matches.\n\n",
    );
    for bucket in BackfillBucket::all() {
        let names: Vec<&str> = uncovered
            .iter()
            .copied()
            .filter(|root| backfill_bucket(root) == *bucket)
            .collect();
        writeln!(section, "### {} ({})\n", bucket.label(), names.len())
            .expect("format backfill heading");
        if names.is_empty() {
            section.push_str("(none)\n\n");
            continue;
        }
        for name in names {
            writeln!(section, "- `{name}`").expect("format uncovered root");
        }
        section.push('\n');
    }
    section
}

fn assert_shader_files_are_compiled_roots(roots: &BTreeSet<String>, files: &BTreeSet<String>) {
    let missing: Vec<&String> = files.difference(roots).collect();
    assert!(
        missing.is_empty(),
        "shader files missing from COMPILED_MATERIAL_STEMS roots: {missing:?}"
    );
}

fn live_snapshot() -> (BTreeSet<String>, CoverageIndex, Vec<AllowEntry>) {
    let roots = shipped_material_roots();
    let files = material_shader_file_stems();
    assert_shader_files_are_compiled_roots(&roots, &files);
    let draws = renderide_test::scene_dsl::material_usage::golden_case_materials();
    let index = index_draws(&roots, &draws).unwrap_or_else(|err| panic!("{err}"));
    let allowlist = load_allowlist(&allowlist_path()).unwrap_or_else(|err| panic!("{err}"));
    (roots, index, allowlist)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(root: &str, reason: &str) -> AllowEntry {
        AllowEntry {
            root: root.to_string(),
            justification: reason.to_string(),
        }
    }

    fn roots(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    fn cases(pairs: &[(&str, &str)]) -> CoverageIndex {
        let mut cases_by_root: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (root, case) in pairs {
            cases_by_root
                .entry((*root).to_string())
                .or_default()
                .insert((*case).to_string());
        }
        CoverageIndex {
            cases_by_root,
            cases_without_shipped_root: BTreeSet::new(),
        }
    }

    #[test]
    fn shader_files_are_compiled_roots_and_keyword_fanout_is_shipped() {
        let roots = shipped_material_roots();
        let files = material_shader_file_stems();
        assert_shader_files_are_compiled_roots(&roots, &files);
        assert!(
            roots.contains("pbsmetallic_alphapremultiply"),
            "keyword fan-out must stay a shipped root"
        );
        assert!(!files.contains("pbsmetallic_alphapremultiply"));
    }

    #[test]
    fn premultiply_variant_bits_select_the_keyword_root() {
        assert_eq!(
            root_for_draw("PBSMetallic.shader", Some(0x0000_0002)),
            "pbsmetallic_alphapremultiply"
        );
        assert_eq!(root_for_draw("PBSMetallic.shader", None), "pbsmetallic");
        assert_eq!(root_for_draw("Unlit.shader", Some(0x0000_0200)), "unlit");
    }

    #[test]
    fn shipped_material_roots_are_covered_or_allowlisted() {
        let (roots, index, allowlist) = live_snapshot();
        check_coverage(&roots, &index, &allowlist, MAX_ALLOWLIST_ROOTS)
            .unwrap_or_else(|err| panic!("{err}"));
    }

    #[test]
    fn material_coverage_doc_matches_sources() {
        let (roots, index, allowlist) = live_snapshot();
        let expected = render_doc(&roots, &index, &allowlist);
        let path = doc_path();
        if std::env::var_os("UPDATE_MATERIAL_COVERAGE").is_some() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap_or_else(|err| panic!("{err}"));
            }
            fs::write(&path, &expected)
                .unwrap_or_else(|err| panic!("write {}: {err}", path.display()));
        }
        // Windows CI checks the file out with CRLF. The generated text is LF.
        let actual = fs::read_to_string(&path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        assert_eq!(
            actual, expected,
            "docs/testing/material-coverage.md drifted; regenerate with {REGENERATE_DOC}"
        );
    }

    #[test]
    fn evidence_dummy_root_without_a_case_is_rejected() {
        let roots = roots(&["unlit", "zz_dummy"]);
        let index = cases(&[("unlit", "torus_unlit_perlin")]);
        let err = check_coverage(&roots, &index, &[], 0).expect_err("dummy root");
        assert_eq!(
            err,
            "uncovered material root `zz_dummy` has no golden case and is not allowlisted"
        );
    }

    #[test]
    fn evidence_dummy_root_passes_once_a_case_exists() {
        let roots = roots(&["zz_dummy"]);
        let index = cases(&[("zz_dummy", "zz_dummy_case")]);
        check_coverage(&roots, &index, &[], 0).expect("covered dummy root");
    }

    #[test]
    fn evidence_stale_allowlist_entry_is_rejected() {
        let roots = roots(&["unlit"]);
        let index = cases(&[("unlit", "torus_unlit_perlin")]);
        let allowlist = [entry("unlit", "was uncovered")];
        let err = check_coverage(&roots, &index, &allowlist, 1).expect_err("stale");
        assert_eq!(
            err,
            "stale allowlist entry `unlit` has golden cases: torus_unlit_perlin"
        );
    }

    #[test]
    fn unknown_allowlist_root_is_rejected() {
        let roots = roots(&["unlit"]);
        let index = cases(&[("unlit", "torus_unlit_perlin")]);
        let allowlist = [entry("not_a_root", "invented")];
        let err = check_coverage(&roots, &index, &allowlist, 1).expect_err("unknown");
        assert_eq!(
            err,
            "allowlist entry `not_a_root` is not a shipped material root"
        );
    }

    #[test]
    fn empty_reason_is_rejected() {
        let err =
            parse_allowlist("[[entry]]\nroot = \"blur\"\nreason = \"\"\n").expect_err("empty");
        assert_eq!(err, "allowlist entry `blur` has an empty reason");
    }

    #[test]
    fn missing_justification_is_rejected() {
        let err = parse_allowlist("[[entry]]\nroot = \"blur\"\n").expect_err("missing");
        assert_eq!(
            err,
            "allowlist entry `blur` needs a reason or an issue link"
        );
    }

    #[test]
    fn issue_link_alone_is_accepted() {
        let entries = parse_allowlist(
            "[[entry]]\nroot = \"blur\"\nissue = \"https://github.com/DoubleStyxx/Renderide/issues/1\"\n",
        )
        .expect("issue link");
        assert_eq!(entries.len(), 1);
        assert!(entries[0].justification.contains("https://"));
    }

    #[test]
    fn allowlist_over_the_max_is_rejected() {
        let roots = roots(&["blur", "gamma"]);
        let index = cases(&[]);
        let allowlist = [entry("blur", "later"), entry("gamma", "later")];
        let err = check_coverage(&roots, &index, &allowlist, 1).expect_err("growth");
        assert!(err.contains("MAX_ALLOWLIST_ROOTS is 1"));
        assert!(err.contains("only allowed edit to that constant is lowering it"));
    }

    #[test]
    fn backfill_buckets_are_disjoint_for_sample_roots() {
        assert_eq!(backfill_bucket("pbsmetallic"), BackfillBucket::Pbs);
        assert_eq!(backfill_bucket("pbsspecular"), BackfillBucket::Pbs);
        assert_eq!(backfill_bucket("paintpbs"), BackfillBucket::Pbs);
        assert_eq!(backfill_bucket("xstoon2.0-cutouta2c"), BackfillBucket::Msaa);
        assert_eq!(
            backfill_bucket("gamma_perobject"),
            BackfillBucket::HdrTonemap
        );
        assert_eq!(backfill_bucket("lut"), BackfillBucket::HdrTonemap);
        assert_eq!(backfill_bucket("textunlit"), BackfillBucket::Text);
        assert_eq!(backfill_bucket("ui_textunlit"), BackfillBucket::Text);
        assert_eq!(backfill_bucket("texturedebug"), BackfillBucket::Remaining);
        assert_eq!(backfill_bucket("billboardunlit"), BackfillBucket::Particles);
    }
}
