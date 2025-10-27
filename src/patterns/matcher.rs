//! This module provides the logic for matching file system paths against glob patterns.
//!
//! The `PatternMatcher` is responsible for compiling glob patterns from the configuration
//! and then checking if a given path matches any of the include or exclude patterns.
//!
//! # Matching Logic
//!
//! The matching process follows these steps:
//! 1.  Check if the path matches any exclusion patterns. If it does, the path is ignored.
//! 2.  If the item is a directory, check it against the directory patterns.
//! 3.  If the item is a file, check it against the file patterns.
//!
//! This order of operations ensures that exclusions always take precedence.

use crate::config::PatternConfig;
use crate::patterns::BUILTIN_PATTERNS;
use crate::types::{PatternCategory, PatternMatch, PatternSource};
use glob::{Pattern, PatternError};
use std::fs::FileType;
use std::path::Path;

/// A matcher that checks paths against compiled glob patterns.
///
/// It holds separate lists of patterns for directories, files, and exclusions.
/// The patterns are pre-compiled into `glob::Pattern` objects for efficient matching.
pub struct PatternMatcher {
    /// Compiled glob patterns for matching directories with their categories.
    directory_patterns: Vec<(Pattern, PatternCategory)>,
    /// Compiled glob patterns for matching files with their categories.
    file_patterns: Vec<(Pattern, PatternCategory)>,
    /// Compiled glob patterns for excluding items.
    exclude_patterns: Vec<Pattern>,
}

impl PatternMatcher {
    /// Creates a new `PatternMatcher` from a `PatternConfig`.
    ///
    /// This method compiles the raw string patterns from the configuration into
    /// efficient `glob::Pattern` objects.
    ///
    /// # Errors
    ///
    /// Returns a `PatternError` if any of the provided glob patterns are invalid.
    pub fn new(config: &PatternConfig) -> Result<Self, PatternError> {
        Ok(Self {
            directory_patterns: Self::compile_patterns_with_categories(&config.directories, true)?,
            file_patterns: Self::compile_patterns_with_categories(&config.files, false)?,
            exclude_patterns: Self::compile_patterns(&config.exclude)?,
        })
    }

    /// Compiles a slice of string patterns into a vector of `glob::Pattern`s.
    fn compile_patterns(patterns: &[String]) -> Result<Vec<Pattern>, PatternError> {
        patterns.iter().map(|p| Pattern::new(p)).collect()
    }

    /// Compiles patterns with their categories by looking them up in BUILTIN_PATTERNS.
    fn compile_patterns_with_categories(
        patterns: &[String],
        _is_dir: bool,
    ) -> Result<Vec<(Pattern, PatternCategory)>, PatternError> {
        patterns
            .iter()
            .map(|p| {
                let pattern = Pattern::new(p)?;
                let category = BUILTIN_PATTERNS.get_category(p);
                Ok((pattern, category))
            })
            .collect()
    }

    /// Checks if a given path matches any of the cleaning patterns.
    ///
    /// It first checks for exclusions. If the path is not excluded, it then checks
    /// against directory and file patterns.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to check.
    ///
    /// # Returns
    ///
    /// An `Option<PatternMatch>` containing details of the match if found, otherwise `None`.
    pub fn matches(&self, path: &Path) -> Option<PatternMatch> {
        match path.symlink_metadata() {
            Ok(metadata) => self.matches_with_type(path, Some(metadata.file_type())),
            Err(_) => self.matches_with_type(path, None),
        }
    }

    /// Checks if a given path matches any of the cleaning patterns using a known file type.
    ///
    /// This variant avoids additional filesystem metadata calls when the caller already
    /// has the `FileType` (e.g., from a directory walk).
    ///
    /// # Arguments
    ///
    /// * `path` - The path to check.
    /// * `file_type` - The file type for the path, typically retrieved from `DirEntry::file_type()`.
    ///
    /// # Returns
    ///
    /// An `Option<PatternMatch>` containing details of the match if found, otherwise `None`.
    pub fn matches_with_type(
        &self,
        path: &Path,
        file_type: Option<FileType>,
    ) -> Option<PatternMatch> {
        // Check exclusions first
        if self.is_excluded(path) {
            return None;
        }

        // Get the file/dir name for matching
        let name = path.file_name()?.to_str()?;

        let (is_dir_candidate, is_file_candidate) = match file_type {
            Some(file_type) => {
                if file_type.is_symlink() {
                    (true, true)
                } else {
                    (file_type.is_dir(), file_type.is_file())
                }
            }
            None => (true, true),
        };

        // Check directory patterns
        if is_dir_candidate {
            for (idx, (pattern, category)) in self.directory_patterns.iter().enumerate() {
                if pattern.matches(name) {
                    return Some(PatternMatch {
                        pattern: pattern.as_str().to_string(),
                        priority: idx as u32,
                        source: PatternSource::Config,
                        category: *category,
                    });
                }
            }
        }

        // Check file patterns
        if is_file_candidate {
            for (idx, (pattern, category)) in self.file_patterns.iter().enumerate() {
                if pattern.matches(name) {
                    return Some(PatternMatch {
                        pattern: pattern.as_str().to_string(),
                        priority: idx as u32,
                        source: PatternSource::Config,
                        category: *category,
                    });
                }
            }
        }

        None
    }

    /// Checks if a path is excluded by any of the exclusion patterns.
    fn is_excluded(&self, path: &Path) -> bool {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            self.exclude_patterns.iter().any(|p| p.matches(name))
        } else {
            false
        }
    }

    /// Adds additional include patterns to the matcher at runtime.
    ///
    /// This is used for merging patterns from the CLI. These patterns are added with
    /// a lower priority than the configuration file patterns.
    ///
    /// # Errors
    ///
    /// Returns a `PatternError` if any of the provided glob patterns are invalid.
    pub fn add_include_patterns(&mut self, patterns: &[String]) -> Result<(), PatternError> {
        for pattern_str in patterns {
            let pattern = Pattern::new(pattern_str)?;
            let category = BUILTIN_PATTERNS.get_category(pattern_str);
            // Try to determine if it's a file or directory pattern
            if pattern_str.contains('.') || pattern_str.contains('*') {
                self.file_patterns.push((pattern, category));
            } else {
                self.directory_patterns.push((pattern, category));
            }
        }
        Ok(())
    }

    /// Adds additional exclude patterns to the matcher at runtime.
    ///
    /// This is used for merging patterns from the CLI.
    ///
    /// # Errors
    ///
    /// Returns a `PatternError` if any of the provided glob patterns are invalid.
    pub fn add_exclude_patterns(&mut self, patterns: &[String]) -> Result<(), PatternError> {
        for pattern_str in patterns {
            self.exclude_patterns.push(Pattern::new(pattern_str)?);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PatternConfig;
    use std::path::Path;

    fn create_matcher(
        directories: Vec<&str>,
        files: Vec<&str>,
        exclude: Vec<&str>,
    ) -> PatternMatcher {
        let config = PatternConfig {
            directories: directories.into_iter().map(String::from).collect(),
            files: files.into_iter().map(String::from).collect(),
            exclude: exclude.into_iter().map(String::from).collect(),
        };
        PatternMatcher::new(&config).unwrap()
    }

    #[test]
    fn test_exclusion_precedence() {
        let matcher = create_matcher(vec!["target"], vec![], vec!["target"]);
        let path = Path::new("target");

        assert!(matcher.matches(path).is_none());
    }

    #[test]
    fn test_directory_and_file_matching() {
        let matcher = create_matcher(vec!["node_modules"], vec!["*.log"], vec![]);
        let dir_path = Path::new("node_modules");
        let file_path = Path::new("app.log");
        let non_match_path = Path::new("src");

        assert!(matcher.matches(dir_path).is_some());
        assert!(matcher.matches(file_path).is_some());
        assert!(matcher.matches(non_match_path).is_none());
    }
}
