//! Code extraction patterns for various documentation frameworks.

/// Code extraction patterns (30+)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodePattern {
    // Documentation frameworks
    DocusaurusCodeBlock,
    DocusaurusTabCodeBlock,
    MkDocsCodeBlock,
    SphinxCodeBlock,
    ReadTheDocsCode,
    JekyllHighlight,
    HugoHighlight,
    VuePressCode,
    GatsbyCode,
    NextjsRehype,
    AstroCode,

    // Syntax highlighters
    PrismJs,
    HighlightJs,
    SyntaxHighlighter,
    RougeSyntax,
    Shiki,

    // Platforms
    GitHubCode,
    GitLabCode,
    BitbucketCode,
    StackOverflowCode,

    // Editors
    MonacoEditor,
    CodeMirror,
    AceEditor,

    // Terminal
    TerminalOutput,
    AsciinemaPlayer,

    // Generic (fallback patterns)
    DataLanguageAttr,
    ClassPrefixCode,
    DataCodeAttr,
    PreCode,
    PreOnly,
    CodeOnly,
}

impl CodePattern {
    /// Get CSS selector for this pattern
    pub fn selector(&self) -> &'static str {
        match self {
            // Documentation frameworks
            Self::DocusaurusCodeBlock => ".prism-code, [class*='codeBlockContent']",
            Self::DocusaurusTabCodeBlock => ".tabs-container pre code",
            Self::MkDocsCodeBlock => ".highlight pre, .codehilite pre",
            Self::SphinxCodeBlock => ".highlight-python pre, .highlight-default pre, .highlight pre",
            Self::ReadTheDocsCode => ".rst-content pre",
            Self::JekyllHighlight => ".highlighter-rouge pre, .highlight pre.highlight",
            Self::HugoHighlight => ".highlight pre, .chroma pre",
            Self::VuePressCode => "div[class*='language-'] pre",
            Self::GatsbyCode => ".gatsby-highlight pre",
            Self::NextjsRehype => "[data-rehype-pretty-code] code",
            Self::AstroCode => ".astro-code pre",

            // Syntax highlighters
            Self::PrismJs => "[class*='language-'] code, pre[class*='language-']",
            Self::HighlightJs => ".hljs, pre code.hljs",
            Self::SyntaxHighlighter => ".syntaxhighlighter",
            Self::RougeSyntax => ".rouge pre, .rouge-code",
            Self::Shiki => ".shiki code, pre.shiki",

            // Platforms
            Self::GitHubCode => ".blob-code-content, .highlight pre, .js-file-line",
            Self::GitLabCode => ".blob-content pre, .code pre",
            Self::BitbucketCode => ".code-container pre",
            Self::StackOverflowCode => ".s-prose pre, .s-code-block, .post-text pre",

            // Editors
            Self::MonacoEditor => ".monaco-editor .view-lines",
            Self::CodeMirror => ".CodeMirror-code, .cm-content",
            Self::AceEditor => ".ace_editor .ace_content",

            // Terminal
            Self::TerminalOutput => ".terminal pre, .console pre, .shell pre",
            Self::AsciinemaPlayer => ".asciinema-player pre",

            // Generic (fallback)
            Self::DataLanguageAttr => "[data-language] code, [data-lang] code",
            Self::ClassPrefixCode => "[class*='code-'] pre, [class*='snippet'] pre",
            Self::DataCodeAttr => "[data-code]",
            Self::PreCode => "pre code",
            Self::PreOnly => "pre:not(:has(code))",
            Self::CodeOnly => "code:not(pre code)",
        }
    }

    /// Get all patterns in priority order (most specific first)
    pub fn all() -> &'static [CodePattern] {
        &[
            // Platform-specific first (most specific)
            Self::GitHubCode,
            Self::GitLabCode,
            Self::BitbucketCode,
            Self::StackOverflowCode,

            // Documentation frameworks
            Self::DocusaurusCodeBlock,
            Self::DocusaurusTabCodeBlock,
            Self::MkDocsCodeBlock,
            Self::SphinxCodeBlock,
            Self::ReadTheDocsCode,
            Self::JekyllHighlight,
            Self::HugoHighlight,
            Self::VuePressCode,
            Self::GatsbyCode,
            Self::NextjsRehype,
            Self::AstroCode,

            // Syntax highlighters
            Self::PrismJs,
            Self::HighlightJs,
            Self::SyntaxHighlighter,
            Self::RougeSyntax,
            Self::Shiki,

            // Editors
            Self::MonacoEditor,
            Self::CodeMirror,
            Self::AceEditor,

            // Terminal
            Self::TerminalOutput,
            Self::AsciinemaPlayer,

            // Generic (fallback, last resort)
            Self::DataLanguageAttr,
            Self::ClassPrefixCode,
            Self::DataCodeAttr,
            Self::PreCode,
            Self::PreOnly,
            Self::CodeOnly,
        ]
    }

    /// Get description of this pattern
    pub fn description(&self) -> &'static str {
        match self {
            Self::DocusaurusCodeBlock => "Docusaurus code block",
            Self::DocusaurusTabCodeBlock => "Docusaurus tabbed code",
            Self::MkDocsCodeBlock => "MkDocs code block",
            Self::SphinxCodeBlock => "Sphinx documentation",
            Self::ReadTheDocsCode => "ReadTheDocs content",
            Self::JekyllHighlight => "Jekyll highlight",
            Self::HugoHighlight => "Hugo highlight",
            Self::VuePressCode => "VuePress code block",
            Self::GatsbyCode => "Gatsby highlight",
            Self::NextjsRehype => "Next.js rehype code",
            Self::AstroCode => "Astro code block",
            Self::PrismJs => "Prism.js",
            Self::HighlightJs => "Highlight.js",
            Self::SyntaxHighlighter => "SyntaxHighlighter",
            Self::RougeSyntax => "Rouge syntax",
            Self::Shiki => "Shiki",
            Self::GitHubCode => "GitHub code view",
            Self::GitLabCode => "GitLab code view",
            Self::BitbucketCode => "Bitbucket code view",
            Self::StackOverflowCode => "Stack Overflow",
            Self::MonacoEditor => "Monaco Editor",
            Self::CodeMirror => "CodeMirror",
            Self::AceEditor => "Ace Editor",
            Self::TerminalOutput => "Terminal output",
            Self::AsciinemaPlayer => "Asciinema",
            Self::DataLanguageAttr => "data-language attribute",
            Self::ClassPrefixCode => "code class prefix",
            Self::DataCodeAttr => "data-code attribute",
            Self::PreCode => "pre > code",
            Self::PreOnly => "pre only",
            Self::CodeOnly => "code only",
        }
    }

    /// Check if this is a generic fallback pattern
    pub fn is_fallback(&self) -> bool {
        matches!(
            self,
            Self::DataLanguageAttr
                | Self::ClassPrefixCode
                | Self::DataCodeAttr
                | Self::PreCode
                | Self::PreOnly
                | Self::CodeOnly
        )
    }

    /// Get pattern name
    pub fn name(&self) -> &'static str {
        match self {
            Self::DocusaurusCodeBlock => "DocusaurusCodeBlock",
            Self::DocusaurusTabCodeBlock => "DocusaurusTabCodeBlock",
            Self::MkDocsCodeBlock => "MkDocsCodeBlock",
            Self::SphinxCodeBlock => "SphinxCodeBlock",
            Self::ReadTheDocsCode => "ReadTheDocsCode",
            Self::JekyllHighlight => "JekyllHighlight",
            Self::HugoHighlight => "HugoHighlight",
            Self::VuePressCode => "VuePressCode",
            Self::GatsbyCode => "GatsbyCode",
            Self::NextjsRehype => "NextjsRehype",
            Self::AstroCode => "AstroCode",
            Self::PrismJs => "PrismJs",
            Self::HighlightJs => "HighlightJs",
            Self::SyntaxHighlighter => "SyntaxHighlighter",
            Self::RougeSyntax => "RougeSyntax",
            Self::Shiki => "Shiki",
            Self::GitHubCode => "GitHubCode",
            Self::GitLabCode => "GitLabCode",
            Self::BitbucketCode => "BitbucketCode",
            Self::StackOverflowCode => "StackOverflowCode",
            Self::MonacoEditor => "MonacoEditor",
            Self::CodeMirror => "CodeMirror",
            Self::AceEditor => "AceEditor",
            Self::TerminalOutput => "TerminalOutput",
            Self::AsciinemaPlayer => "AsciinemaPlayer",
            Self::DataLanguageAttr => "DataLanguageAttr",
            Self::ClassPrefixCode => "ClassPrefixCode",
            Self::DataCodeAttr => "DataCodeAttr",
            Self::PreCode => "PreCode",
            Self::PreOnly => "PreOnly",
            Self::CodeOnly => "CodeOnly",
        }
    }

    /// Get example sites for this pattern
    pub fn example_sites(&self) -> &'static [&'static str] {
        match self {
            Self::DocusaurusCodeBlock | Self::DocusaurusTabCodeBlock => &["docusaurus.io", "reactjs.org", "create-react-app.dev"],
            Self::MkDocsCodeBlock => &["mkdocs.org", "squidfunk.github.io/mkdocs-material"],
            Self::SphinxCodeBlock => &["sphinx-doc.org", "docs.python.org"],
            Self::ReadTheDocsCode => &["readthedocs.org"],
            Self::JekyllHighlight => &["jekyllrb.com", "github.io"],
            Self::HugoHighlight => &["gohugo.io"],
            Self::VuePressCode => &["vuepress.vuejs.org", "vuejs.org"],
            Self::GatsbyCode => &["gatsbyjs.com"],
            Self::NextjsRehype => &["nextjs.org"],
            Self::AstroCode => &["astro.build"],
            Self::PrismJs => &["prismjs.com"],
            Self::HighlightJs => &["highlightjs.org"],
            Self::SyntaxHighlighter => &["alexgorbatchev.com/SyntaxHighlighter"],
            Self::RougeSyntax => &["rouge.jneen.net"],
            Self::Shiki => &["shiki.matsu.io"],
            Self::GitHubCode => &["github.com"],
            Self::GitLabCode => &["gitlab.com"],
            Self::BitbucketCode => &["bitbucket.org"],
            Self::StackOverflowCode => &["stackoverflow.com"],
            Self::MonacoEditor => &["microsoft.github.io/monaco-editor"],
            Self::CodeMirror => &["codemirror.net"],
            Self::AceEditor => &["ace.c9.io"],
            Self::TerminalOutput | Self::AsciinemaPlayer => &["asciinema.org"],
            _ => &[], // Generic patterns have no specific sites
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_patterns_count() {
        // Should have 30+ patterns
        assert!(CodePattern::all().len() >= 30);
    }

    #[test]
    fn test_all_patterns_have_selectors() {
        for pattern in CodePattern::all() {
            let selector = pattern.selector();
            assert!(!selector.is_empty(), "Pattern {:?} has empty selector", pattern);
        }
    }

    #[test]
    fn test_pattern_descriptions() {
        for pattern in CodePattern::all() {
            let desc = pattern.description();
            assert!(!desc.is_empty(), "Pattern {:?} has empty description", pattern);
        }
    }

    #[test]
    fn test_fallback_patterns() {
        assert!(CodePattern::PreCode.is_fallback());
        assert!(CodePattern::CodeOnly.is_fallback());
        assert!(!CodePattern::GitHubCode.is_fallback());
        assert!(!CodePattern::DocusaurusCodeBlock.is_fallback());
    }
}
