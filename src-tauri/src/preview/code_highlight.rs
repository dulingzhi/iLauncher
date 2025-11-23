use anyhow::Result;
use std::fs;
use std::path::Path;
use syntect::easy::HighlightLines;
use syntect::highlighting::{ThemeSet, Style};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// 代码高亮增强模块
pub struct CodeHighlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl CodeHighlighter {
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    /// 高亮代码文件
    pub fn highlight_file(&self, file_path: &Path) -> Result<String> {
        let content = fs::read_to_string(file_path)?;
        let extension = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt");
        
        self.highlight_code(&content, extension)
    }

    /// 高亮代码字符串
    pub fn highlight_code(&self, code: &str, extension: &str) -> Result<String> {
        // 查找语法定义
        let syntax = self.syntax_set
            .find_syntax_by_extension(extension)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        
        // 使用深色主题
        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);
        
        // 生成HTML
        let mut html = String::from(r#"<pre style="background: #2b303b; color: #c0c5ce; padding: 12px; border-radius: 6px; overflow-x: auto; font-family: 'Consolas', 'Monaco', monospace; font-size: 13px; line-height: 1.5;"><code>"#);
        
        for (i, line) in LinesWithEndings::from(code).enumerate() {
            // 添加行号
            html.push_str(&format!(
                r#"<span style="color: #65737e; user-select: none; margin-right: 12px;">{:>4}</span>"#,
                i + 1
            ));
            
            // 高亮代码
            let ranges = highlighter.highlight_line(line, &self.syntax_set)?;
            let escaped = Self::ranges_to_html(&ranges);
            html.push_str(&escaped);
        }
        
        html.push_str("</code></pre>");
        Ok(html)
    }

    /// 转换样式范围为HTML
    fn ranges_to_html(ranges: &[(Style, &str)]) -> String {
        let mut html = String::new();
        for (style, text) in ranges {
            let fg = style.foreground;
            let color = format!("#{:02x}{:02x}{:02x}", fg.r, fg.g, fg.b);
            
            if style.font_style.contains(syntect::highlighting::FontStyle::BOLD) {
                html.push_str(&format!(r#"<strong style="color: {};">{}</strong>"#, color, Self::escape_html(text)));
            } else if style.font_style.contains(syntect::highlighting::FontStyle::ITALIC) {
                html.push_str(&format!(r#"<em style="color: {};">{}</em>"#, color, Self::escape_html(text)));
            } else {
                html.push_str(&format!(r#"<span style="color: {};">{}</span>"#, color, Self::escape_html(text)));
            }
        }
        html
    }

    /// HTML转义
    fn escape_html(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;")
    }

    /// 获取支持的语言列表
    pub fn supported_languages(&self) -> Vec<String> {
        self.syntax_set
            .syntaxes()
            .iter()
            .map(|s| s.name.clone())
            .collect()
    }

    /// 检测文件是否支持高亮
    pub fn is_supported(&self, extension: &str) -> bool {
        self.syntax_set.find_syntax_by_extension(extension).is_some()
    }
}

impl Default for CodeHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_rust() {
        let highlighter = CodeHighlighter::new();
        let code = r#"fn main() {
    println!("Hello, world!");
}"#;
        let html = highlighter.highlight_code(code, "rs").unwrap();
        assert!(html.contains("<pre"));
        assert!(html.contains("fn"));
        assert!(html.contains("println!"));
    }

    #[test]
    fn test_supported_languages() {
        let highlighter = CodeHighlighter::new();
        let languages = highlighter.supported_languages();
        assert!(languages.contains(&"Rust".to_string()));
        assert!(languages.contains(&"Python".to_string()));
        assert!(languages.contains(&"JavaScript".to_string()));
    }

    #[test]
    fn test_is_supported() {
        let highlighter = CodeHighlighter::new();
        assert!(highlighter.is_supported("rs"));
        assert!(highlighter.is_supported("py"));
        assert!(highlighter.is_supported("js"));
        assert!(!highlighter.is_supported("xyz123"));
    }
}
