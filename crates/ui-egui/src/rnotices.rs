//! Translated notices for the R runner's English errors.

/// `text` translated, when it is one of the R notices (with or without the prefix). `None` for any
/// other text.
#[cfg(feature = "r")]
pub fn translate(text: &str) -> Option<String> {
    use rustgis_engine::cmd::gp::R_FAILURE_PREFIX;
    use rustgis_engine::r::notices::{NEED_JSONLITE, NEED_SF, NEED_TERRA, NOT_FOUND};

    let text = text.strip_prefix(R_FAILURE_PREFIX).unwrap_or(text);
    let translated = match text {
        NOT_FOUND => tl!(NOT_FOUND),
        NEED_JSONLITE => tl!(NEED_JSONLITE),
        NEED_SF => tl!(NEED_SF),
        NEED_TERRA => tl!(NEED_TERRA),
        _ => return None,
    };
    Some(translated.to_string())
}

/// Without R there are no R notices to translate.
#[cfg(not(feature = "r"))]
pub fn translate(_text: &str) -> Option<String> {
    None
}

#[cfg(all(test, feature = "r"))]
mod tests {
    use super::*;
    use crate::i18n::{Lang, tr};
    use rustgis_engine::cmd::gp::R_FAILURE_PREFIX;
    use rustgis_engine::r::notices::ALL;

    #[test]
    fn each_notice_maps_with_and_without_the_prefix() {
        for text in ALL {
            assert!(translate(text).is_some(), "{text}");
            assert_eq!(translate(&format!("{R_FAILURE_PREFIX}{text}")), translate(text), "{text}");
        }
    }

    #[test]
    fn other_text_is_not_translated() {
        assert_eq!(translate("R exited with 1: Execution halted"), None);
        assert_eq!(translate(&format!("{R_FAILURE_PREFIX}something else")), None);
        assert_eq!(translate(""), None);
    }

    #[test]
    fn english_notice_matches_the_catalog_key() {
        assert_eq!(translate("Rscript was not found").as_deref(), Some(tr(Lang::EN, "Rscript was not found")));
    }
}
