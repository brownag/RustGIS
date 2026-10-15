//! The English notices the desktop translates. Keep them in step with `bootstrap.R`, which holds
//! the three R-side texts (with `"` written as `\"`).

/// The `Rscript` program was not found.
pub const NOT_FOUND: &str = "Rscript was not found";

/// `jsonlite` is missing, so the script cannot write its results.
pub const NEED_JSONLITE: &str = "RustGIS needs the R package 'jsonlite'; install it with install.packages(\"jsonlite\")";

/// The script needs `sf` for geometry.
pub const NEED_SF: &str = "this script needs geometry; install the R package 'sf': install.packages(\"sf\")";

/// The script needs `terra` for rasters.
pub const NEED_TERRA: &str = "this script needs rasters; install the R package 'terra': install.packages(\"terra\")";

/// Every notice above.
pub const ALL: [&str; 4] = [NOT_FOUND, NEED_JSONLITE, NEED_SF, NEED_TERRA];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RRunnerError;
    use crate::harness::BOOTSTRAP_R;

    #[test]
    fn r_side_notices_match_bootstrap() {
        for text in [NEED_JSONLITE, NEED_SF, NEED_TERRA] {
            let escaped = text.replace('"', "\\\"");
            assert!(BOOTSTRAP_R.contains(&escaped), "bootstrap.R lacks: {text}");
        }
        let escaped_sf = NEED_SF.replace('"', "\\\"");
        assert!(BOOTSTRAP_R.matches(&escaped_sf).count() >= 2);
    }

    #[test]
    fn not_found_error_uses_the_constant() {
        assert_eq!(RRunnerError::NotFound.to_string(), NOT_FOUND);
    }
}
