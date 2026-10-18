//! Translated notices for the R runner's typed notices.

use rustgis_engine::Notice;

/// The text to show for a notice: the translated R notice when it has one, else its own text.
#[cfg(feature = "r")]
pub fn text_of(n: &Notice) -> String {
    match n.r_notice {
        Some(r) => text(r),
        None => n.text.clone(),
    }
}

/// Without R there are no R notices: a notice's own text.
#[cfg(not(feature = "r"))]
pub fn text_of(n: &Notice) -> String {
    n.text.clone()
}

/// A typed R notice, translated.
#[cfg(feature = "r")]
fn text(n: rustgis_engine::r::RNotice) -> String {
    use rustgis_engine::r::RNotice;
    use rustgis_engine::r::notices::{NEED_JSONLITE, NEED_SF, NEED_TERRA, NOT_FOUND};
    let translated = match n {
        RNotice::NotFound => tl!(NOT_FOUND),
        RNotice::NeedJsonlite => tl!(NEED_JSONLITE),
        RNotice::NeedSf => tl!(NEED_SF),
        RNotice::NeedTerra => tl!(NEED_TERRA),
    };
    translated.to_string()
}

#[cfg(all(test, feature = "r"))]
mod tests {
    use super::*;
    use crate::i18n::{Lang, tr};
    use rustgis_engine::r::notices::{ALL_NOTICES, NOT_FOUND};

    fn notice(text: &str, r: Option<rustgis_engine::r::RNotice>) -> Notice {
        Notice { text: text.into(), error: true, r_notice: r }
    }

    #[test]
    fn each_typed_notice_is_translated_from_its_english() {
        for n in ALL_NOTICES {
            assert_eq!(text_of(&notice(n.english(), Some(n))), text(n), "{}", n.code());
        }
    }

    #[test]
    fn untyped_text_is_kept() {
        assert_eq!(text_of(&notice("R exited with 1: Execution halted", None)), "R exited with 1: Execution halted");
        assert_eq!(text_of(&notice("", None)), "");
    }

    #[test]
    fn english_notice_matches_the_catalog_key() {
        assert_eq!(text(rustgis_engine::r::RNotice::NotFound), tr(Lang::EN, NOT_FOUND));
    }
}
