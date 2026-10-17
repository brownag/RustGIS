//! The notices the desktop translates. Each has a stable code (what bootstrap.R and the manifest
//! carry) and an English text (kept in step with `bootstrap.R`, which holds the R-side texts with
//! `"` written as `\"`).

/// The `Rscript` program was not found.
pub const NOT_FOUND: &str = "Rscript was not found";

/// `jsonlite` is missing, so the script cannot write its results.
pub const NEED_JSONLITE: &str = "RustGIS needs the R package 'jsonlite'; install it with install.packages(\"jsonlite\")";

/// The script needs `sf` for geometry.
pub const NEED_SF: &str = "this script needs geometry; install the R package 'sf': install.packages(\"sf\")";

/// The script needs `terra` for rasters.
pub const NEED_TERRA: &str = "this script needs rasters; install the R package 'terra': install.packages(\"terra\")";

/// A notice the R runner can report, typed so the front end translates it without matching text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RNotice {
    NotFound,
    NeedJsonlite,
    NeedSf,
    NeedTerra,
}

/// Every notice.
pub const ALL_NOTICES: [RNotice; 4] = [RNotice::NotFound, RNotice::NeedJsonlite, RNotice::NeedSf, RNotice::NeedTerra];

impl RNotice {
    /// The stable code (bootstrap.R writes it into the manifest).
    pub fn code(self) -> &'static str {
        match self {
            RNotice::NotFound => "not_found",
            RNotice::NeedJsonlite => "need_jsonlite",
            RNotice::NeedSf => "need_sf",
            RNotice::NeedTerra => "need_terra",
        }
    }

    /// The English text.
    pub fn english(self) -> &'static str {
        match self {
            RNotice::NotFound => NOT_FOUND,
            RNotice::NeedJsonlite => NEED_JSONLITE,
            RNotice::NeedSf => NEED_SF,
            RNotice::NeedTerra => NEED_TERRA,
        }
    }

    /// The notice with this code, if any.
    pub fn from_code(code: &str) -> Option<Self> {
        ALL_NOTICES.into_iter().find(|n| n.code() == code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RRunnerError;
    use crate::harness::BOOTSTRAP_R;

    #[test]
    fn codes_round_trip() {
        for n in ALL_NOTICES {
            assert_eq!(RNotice::from_code(n.code()), Some(n));
        }
        assert_eq!(RNotice::from_code("bogus"), None);
        assert_eq!(RNotice::from_code(""), None);
    }

    #[test]
    fn r_side_codes_and_notices_match_bootstrap() {
        // not_found is raised by the runner, not by the R script.
        for n in [RNotice::NeedJsonlite, RNotice::NeedSf, RNotice::NeedTerra] {
            assert!(BOOTSTRAP_R.contains(n.code()), "bootstrap.R lacks code {}", n.code());
            let escaped = n.english().replace('"', "\\\"");
            assert!(BOOTSTRAP_R.contains(&escaped), "bootstrap.R lacks: {}", n.english());
        }
    }

    #[test]
    fn not_found_error_uses_the_constant() {
        assert_eq!(RRunnerError::NotFound.to_string(), NOT_FOUND);
        assert_eq!(RRunnerError::Notice(RNotice::NeedSf).to_string(), NEED_SF);
    }
}
