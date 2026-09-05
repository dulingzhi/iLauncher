//! Markdown 块级解析（pulldown-cmark → 可测 AST，无 gpui 依赖）。
//!
//! AIChat 消息渲染用：引擎把消息文本解析成块序列，UI 逐块映射为 gpui 元素。
//! 语法高亮不做（方案允许降级：代码块按等宽 + 语言标签渲染）。

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// 行内样式组合位
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InlineStyle {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    /// 链接目标 URL（行内代码内的链接标记不生效）
    pub link: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MdInline {
    pub text: String,
    pub style: InlineStyle,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MdBlock {
    Heading(u8, Vec<MdInline>),
    Paragraph(Vec<MdInline>),
    /// 语言标签可能为空串
    Code { lang: String, text: String },
    List { ordered: bool, items: Vec<Vec<MdInline>> },
    Quote(Vec<MdInline>),
    Rule,
}

/// 解析 Markdown 为块序列（空输入返回空Vec，不报错）
pub fn parse_blocks(src: &str) -> Vec<MdBlock> {
    /// 挂起文本按记录样式落盘
    fn push_run(buf: &mut Vec<MdInline>, text: &mut String, style: &InlineStyle) {
        if !text.is_empty() {
            buf.push(MdInline { text: std::mem::take(text), style: style.clone() });
        }
    }

    let mut blocks = Vec::new();
    // 当前块累积状态：cur_text 是未落盘的挂起文本，cur_style 是它被写入时的样式
    let mut inline_buf: Vec<MdInline> = Vec::new();
    let mut cur_text = String::new();
    let mut cur_style = InlineStyle::default();
    // 样式栈（嵌套 strong/em）与链接目标（作用于后续文本）
    let mut bold_depth = 0usize;
    let mut italic_depth = 0usize;
    let mut link_target: Option<String> = None;

    // 列表/引用等需要"块结束时投递"的暂存
    let mut list_items: Vec<Vec<MdInline>> = Vec::new();
    let mut list_ordered = false;
    let mut cur_item: Option<Vec<MdInline>> = None;
    let mut quote_buf: Option<Vec<MdInline>> = None;

    // 行内文本的落盘目标：引用 > 列表项 > 普通块缓冲
    macro_rules! target {
        () => {
            if let Some(q) = quote_buf.as_mut() {
                &mut *q
            } else if let Some(item) = cur_item.as_mut() {
                &mut *item
            } else {
                &mut inline_buf
            }
        };
    }

    let parser = Parser::new_ext(src, Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS);
    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    inline_buf.clear();
                    cur_text.clear();
                    cur_style = InlineStyle::default();
                    bold_depth = 0;
                    italic_depth = 0;
                    blocks.push(MdBlock::Heading(level as u8, Vec::new()));
                }
                Tag::Paragraph => {
                    inline_buf.clear();
                    cur_text.clear();
                    cur_style = InlineStyle::default();
                    bold_depth = 0;
                    italic_depth = 0;
                }
                Tag::CodeBlock(kind) => {
                    let lang = match kind {
                        pulldown_cmark::CodeBlockKind::Fenced(l) => l.to_string(),
                        pulldown_cmark::CodeBlockKind::Indented => String::new(),
                    };
                    blocks.push(MdBlock::Code { lang, text: String::new() });
                }
                Tag::List(start) => {
                    list_ordered = start.is_some();
                    list_items.clear();
                }
                Tag::Item => {
                    cur_item = Some(Vec::new());
                    cur_text.clear();
                    cur_style = InlineStyle::default();
                    bold_depth = 0;
                    italic_depth = 0;
                }
                Tag::BlockQuote(_) => {
                    quote_buf = Some(Vec::new());
                    cur_text.clear();
                    cur_style = InlineStyle::default();
                    bold_depth = 0;
                    italic_depth = 0;
                }
                Tag::Strong => bold_depth += 1,
                Tag::Emphasis => italic_depth += 1,
                Tag::Link { dest_url, .. } => link_target = Some(dest_url.to_string()),
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    push_run(&mut inline_buf, &mut cur_text, &cur_style);
                    if let Some(MdBlock::Heading(_, buf)) = blocks.last_mut() {
                        *buf = std::mem::take(&mut inline_buf);
                    }
                }
                TagEnd::Paragraph => {
                    if let Some(q) = quote_buf.as_mut() {
                        push_run(q, &mut cur_text, &cur_style);
                    } else {
                        push_run(&mut inline_buf, &mut cur_text, &cur_style);
                        if !inline_buf.is_empty() {
                            blocks.push(MdBlock::Paragraph(std::mem::take(&mut inline_buf)));
                        }
                    }
                }
                TagEnd::CodeBlock => {}
                TagEnd::Item => {
                    if let Some(item) = cur_item.as_mut() {
                        push_run(item, &mut cur_text, &cur_style);
                    }
                    if let Some(item) = cur_item.take() {
                        list_items.push(item);
                    }
                }
                TagEnd::List(_) => {
                    blocks.push(MdBlock::List {
                        ordered: list_ordered,
                        items: std::mem::take(&mut list_items),
                    });
                }
                TagEnd::BlockQuote(_) => {
                    if let Some(q) = quote_buf.as_mut() {
                        push_run(q, &mut cur_text, &cur_style);
                    }
                    if let Some(q) = quote_buf.take() {
                        blocks.push(MdBlock::Quote(q));
                    }
                }
                TagEnd::Strong => bold_depth = bold_depth.saturating_sub(1),
                TagEnd::Emphasis => italic_depth = italic_depth.saturating_sub(1),
                TagEnd::Link => link_target = None,
                _ => {}
            },
            Event::Text(text) => {
                // 代码块：原文累积到最后一个 Code 块
                if matches!(blocks.last(), Some(MdBlock::Code { .. })) {
                    if let Some(MdBlock::Code { text: code, .. }) = blocks.last_mut() {
                        code.push_str(&text);
                    }
                    continue;
                }
                // 挂起文本样式变化时先按旧样式落盘，再换新样式
                let new_style = InlineStyle {
                    bold: bold_depth > 0,
                    italic: italic_depth > 0,
                    link: link_target.clone(),
                    code: false,
                };
                if cur_text.is_empty() {
                    cur_style = new_style;
                } else if new_style != cur_style {
                    push_run(target!(), &mut cur_text, &cur_style);
                    cur_style = new_style;
                }
                cur_text.push_str(&text);
            }
            Event::Code(text) => {
                // 行内代码是独立 run：先落盘挂起文本
                push_run(target!(), &mut cur_text, &cur_style);
                target!().push(MdInline {
                    text: text.to_string(),
                    style: InlineStyle { code: true, ..Default::default() },
                });
            }
            Event::Rule => blocks.push(MdBlock::Rule),
            Event::SoftBreak | Event::HardBreak => {
                cur_text.push('\n');
            }
            _ => {}
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(blocks: &[MdBlock]) -> Vec<String> {
        blocks
            .iter()
            .map(|b| match b {
                MdBlock::Heading(_, runs) | MdBlock::Paragraph(runs) | MdBlock::Quote(runs) => {
                    runs.iter().map(|r| r.text.clone()).collect::<String>()
                }
                MdBlock::Code { text, .. } => text.clone(),
                MdBlock::List { items, .. } => {
                    items.iter().map(|i| i.iter().map(|r| r.text.clone()).collect::<String>()).collect::<Vec<_>>().join("|")
                }
                MdBlock::Rule => "---".into(),
            })
            .collect()
    }

    #[test]
    fn empty_input() {
        assert!(parse_blocks("").is_empty());
        assert!(parse_blocks("  \n ").is_empty());
    }

    #[test]
    fn heading_levels() {
        let blocks = parse_blocks("# 标题一\n## 标题二");
        assert!(matches!(&blocks[0], MdBlock::Heading(1, _)));
        assert!(matches!(&blocks[1], MdBlock::Heading(2, _)));
        assert_eq!(plain(&blocks), vec!["标题一", "标题二"]);
    }

    #[test]
    fn paragraph_and_softbreak() {
        let blocks = parse_blocks("第一行\n第二行");
        assert_eq!(blocks.len(), 1);
        assert_eq!(plain(&blocks), vec!["第一行\n第二行"]);
    }

    #[test]
    fn bold_italic_runs() {
        let blocks = parse_blocks("普通**加粗**又*斜体*尾");
        let MdBlock::Paragraph(runs) = &blocks[0] else { panic!() };
        let snapshot: Vec<(&str, bool, bool)> =
            runs.iter().map(|r| (r.text.as_str(), r.style.bold, r.style.italic)).collect();
        assert_eq!(
            snapshot,
            vec![("普通", false, false), ("加粗", true, false), ("又", false, false), ("斜体", false, true), ("尾", false, false)]
        );
    }

    #[test]
    fn fenced_code_block() {
        let blocks = parse_blocks("```rust\nfn main() {}\n```");
        assert_eq!(blocks.len(), 1);
        let MdBlock::Code { lang, text } = &blocks[0] else { panic!() };
        assert_eq!(lang, "rust");
        assert_eq!(text.trim(), "fn main() {}");
    }

    #[test]
    fn inline_code_style() {
        let blocks = parse_blocks("用 `cargo test` 运行");
        let MdBlock::Paragraph(runs) = &blocks[0] else { panic!() };
        assert!(runs.iter().any(|r| r.style.code && r.text.contains("cargo test")));
    }

    #[test]
    fn unordered_list() {
        let blocks = parse_blocks("- 甲\n- 乙");
        let MdBlock::List { ordered, items } = &blocks[0] else { panic!() };
        assert!(!ordered);
        assert_eq!(items.len(), 2);
        assert_eq!(plain(&blocks), vec!["甲|乙"]);
    }

    #[test]
    fn ordered_list() {
        let blocks = parse_blocks("1. 一\n2. 二");
        let MdBlock::List { ordered, items } = &blocks[0] else { panic!() };
        assert!(ordered);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn quote_block() {
        let blocks = parse_blocks("> 引用内容");
        assert!(matches!(&blocks[0], MdBlock::Quote(_)));
        assert_eq!(plain(&blocks), vec!["引用内容"]);
    }

    #[test]
    fn horizontal_rule() {
        let blocks = parse_blocks("上\n\n---\n\n下");
        assert!(blocks.iter().any(|b| matches!(b, MdBlock::Rule)));
    }

    #[test]
    fn link_style_recorded() {
        let blocks = parse_blocks("[文档](https://example.com)");
        let MdBlock::Paragraph(runs) = &blocks[0] else { panic!() };
        assert_eq!(runs[0].style.link.as_deref(), Some("https://example.com"));
    }
}
