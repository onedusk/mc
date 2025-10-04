use glob::{Pattern, PatternError};
use std::path::Path;
use crate::types::{PatternMatch, PatternSource};
use crate::config::PatternConfig;

pub struct PatternMatcher {
    directory_patterns: Vec<Pattern>,
    file_patterns: Vec<Pattern>,
    exclude_patterns: Vec<Pattern>,
}

impl PatternMatcher {
    pub fn new(config: &PatternConfig) -> Result<Self, PatternError> {
        Ok(Self {
            directory_patterns: Self::compile_patterns(&config.directories)?,
            file_patterns: Self::compile_patterns(&config.files)?,
            exclude_patterns: Self::compile_patterns(&config.exclude)?,
        })
    }

    fn compile_patterns(patterns: &[String]) -> Result<Vec<Pattern>, PatternError> {
        patterns.iter()
            .map(|p| Pattern::new(p))
            .collect()
    }

    pub fn matches(&self, path: &Path) -> Option<PatternMatch> {
        // Check exclusions first
        if self.is_excluded(path) {
            return None;
        }

        // Get the file/dir name for matching
        let name = path.file_name()?.to_str()?;

        // Check directory patterns
        if path.is_dir() {
            for (idx, pattern) in self.directory_patterns.iter().enumerate() {
                if pattern.matches(name) {
                    return Some(PatternMatch {
                        pattern: pattern.as_str().to_string(),
                        priority: idx as u32,
                        source: PatternSource::Config,
                    });
                }
            }
        }

        // Check file patterns
        if path.is_file() {
            for (idx, pattern) in self.file_patterns.iter().enumerate() {
                if pattern.matches(name) {
                    return Some(PatternMatch {
                        pattern: pattern.as_str().to_string(),
                        priority: idx as u32,
                        source: PatternSource::Config,
                    });
                }
            }
        }

        None
    }

    fn is_excluded(&self, path: &Path) -> bool {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            self.exclude_patterns.iter()
                .any(|p| p.matches(name))
        } else {
            false
        }
    }

    pub fn add_include_patterns(&mut self, patterns: &[String]) -> Result<(), PatternError> {
        for pattern_str in patterns {
            let pattern = Pattern::new(pattern_str)?;
            // Try to determine if it's a file or directory pattern
            if pattern_str.contains('.') || pattern_str.contains('*') {
                self.file_patterns.push(pattern);
            } else {
                self.directory_patterns.push(pattern);
            }
        }
        Ok(())
    }

    pub fn add_exclude_patterns(&mut self, patterns: &[String]) -> Result<(), PatternError> {
        for pattern_str in patterns {
            self.exclude_patterns.push(Pattern::new(pattern_str)?);
        }
        Ok(())
    }
}