use once_cell::sync::Lazy;

pub static BUILTIN_PATTERNS: Lazy<PatternSet> = Lazy::new(|| {
    PatternSet {
        directories: vec![
            // Build outputs
            "dist", "build", ".next", "out", "target",
            // Dependencies
            "node_modules", ".venv", "vendor",
            // Cache
            ".turbo", ".bun", ".pytest_cache", ".benchmark-cache",
            "coverage", ".ropeproject", ".ruby-lsp",
            // Tools
            ".idea", ".flock", ".swarm", ".hive-mind",
            ".claude-flow", ".roo", "memory", "coordination",
            // Additional from the shell script
            "claude-flow", ".mcp.json",
        ],
        files: vec![
            "*.log",
            "*.tsbuildinfo",
            "package-lock.json",
            "bun.lock",
            "uv.lock",
            "Gemfile.lock",
            "claude-flow.bat",
            "claude-flow.ps1",
            "claude-flow.config.json",
            "AGENTS.md",
            "claude-flow-1.0.70.tgz",
        ],
        exclude: vec![
            ".git",
            ".github",
        ],
    }
});

pub struct PatternSet {
    pub directories: Vec<&'static str>,
    pub files: Vec<&'static str>,
    pub exclude: Vec<&'static str>,
}