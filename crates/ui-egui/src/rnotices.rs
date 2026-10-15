//! Translated notices for the R runner's English errors.

/// The prefix `r_complete` adds to every R failure.
const PREFIX: &str = "R script: ";

/// `text` translated, when it is one of the R notices (with or without the prefix). `None` for any
/// other text.
pub fn translate(text: &str) -> Option<String> {
    let text = text.strip_prefix(PREFIX).unwrap_or(text);
    let translated = match text {
        "Rscript was not found" => tl!("Rscript was not found"),
        "RustGIS needs the R package 'jsonlite'; install it with install.packages(\"jsonlite\")" => {
            tl!("RustGIS needs the R package 'jsonlite'; install it with install.packages(\"jsonlite\")")
        }
        "this script needs geometry; install the R package 'sf': install.packages(\"sf\")" => {
            tl!("this script needs geometry; install the R package 'sf': install.packages(\"sf\")")
        }
        "this script needs rasters; install the R package 'terra': install.packages(\"terra\")" => {
            tl!("this script needs rasters; install the R package 'terra': install.packages(\"terra\")")
        }
        _ => return None,
    };
    Some(translated.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Lang, tr};

    const TEXTS: [&str; 4] = [
        "Rscript was not found",
        "RustGIS needs the R package 'jsonlite'; install it with install.packages(\"jsonlite\")",
        "this script needs geometry; install the R package 'sf': install.packages(\"sf\")",
        "this script needs rasters; install the R package 'terra': install.packages(\"terra\")",
    ];

    #[test]
    fn each_notice_maps_with_and_without_the_prefix() {
        for text in TEXTS {
            assert!(translate(text).is_some(), "{text}");
            assert_eq!(translate(&format!("R script: {text}")), translate(text), "{text}");
        }
    }

    #[test]
    fn other_text_is_not_translated() {
        assert_eq!(translate("R exited with 1: Execution halted"), None);
        assert_eq!(translate("R script: something else"), None);
        assert_eq!(translate(""), None);
    }

    #[test]
    fn english_notice_matches_the_catalog_key() {
        assert_eq!(translate("Rscript was not found").as_deref(), Some(tr(Lang::EN, "Rscript was not found")));
    }
}
