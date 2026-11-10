use super::score_text;

#[test]
fn ranks_candidates_without_precomputed_fields() {
    let candidate: crate::model::Candidate =
        serde_json::from_str(r#"{"id":7,"title":"Google Chrome"}"#).unwrap();
    let results = super::rank_candidates("chrome", &[candidate], 10);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 7);
}

#[test]
fn fuzzy_boundary_bonus_uses_characters_for_unicode_titles() {
    assert_eq!(
        score_text("ace", "日本 abcd ef"),
        score_text("ace", "abcd ef")
    );
}

#[test]
fn ranks_canonical_product_above_compound_modifier_titles() {
    let chrome_main = score_text("chrome", "Google Chrome").unwrap();
    let chrome_pwa = score_text("chrome", "Chrome Remote Desktop").unwrap();
    assert!(
        chrome_main > chrome_pwa,
        "Google Chrome ({}) should beat Chrome Remote Desktop ({})",
        chrome_main,
        chrome_pwa
    );
}

#[test]
fn ranks_exact_and_prefix_matches_above_fuzzy_matches() {
    assert!(score_text("chrome", "Chrome").unwrap() > score_text("chr", "Google Chrome").unwrap());
    assert!(score_text("gch", "Google Chrome").is_some());
}

#[test]
fn rejects_short_scattered_matches() {
    assert!(score_text("ce", "Google Chrome").is_none());
    assert!(score_text("chr", "Google Chrome").is_some());
}

#[test]
fn handles_separator_normalization_and_acronyms() {
    assert!(score_text("control panel", "Control_Panel").is_some());
    assert!(score_text("cp", "Control Panel").is_some());
}

#[test]
fn handles_camel_case_and_compound_file_acronyms() {
    assert!(score_text("rsb", "RustSearchBackend").is_some());
    assert!(score_text("qr", "quarterly_report_2026.docx").is_some());
    assert!(score_text("oss", "OmniSpotSettings").is_some());
    assert!(score_text("upc", "user-profile-component.ts").is_some());
}
