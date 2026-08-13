//! Markdown outline extraction for the read-only document view.

use markdown::{ParseOptions, mdast::Node, to_mdast};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OutlineEntry {
    pub level: u8,
    pub text: String,
    /// Index of the rendered section in the document scroll container.
    pub section: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MarkdownSection {
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParsedDocument {
    pub entries: Vec<OutlineEntry>,
    pub sections: Vec<MarkdownSection>,
}

/// Parse h1–h3 headings and split the document at top-level heading boundaries.
///
/// Each section remains valid Markdown and is rendered by the existing GPUI
/// TextView. The split gives the outer scroll container stable children, which
/// lets the TOC use ScrollHandle's item positioning without replacing the
/// project's Markdown renderer.
pub(crate) fn parse_document(source: &str) -> ParsedDocument {
    let front_matter_end = yaml_front_matter_end(source).unwrap_or_default();
    let headings = to_mdast(source, &ParseOptions::default())
        .ok()
        .and_then(|root| match root {
            Node::Root(root) => Some(
                root.children
                    .iter()
                    .filter_map(|node| match node {
                        Node::Heading(heading) if heading.depth <= 3 => heading
                            .position
                            .as_ref()
                            .filter(|position| position.start.offset >= front_matter_end)
                            .map(|position| {
                                (
                                    position.start.offset,
                                    heading.depth,
                                    heading_text(&heading.children),
                                )
                            }),
                        _ => None,
                    })
                    .filter(|(_, _, text)| !text.is_empty())
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .unwrap_or_default();

    if headings.is_empty() {
        return ParsedDocument {
            entries: Vec::new(),
            sections: vec![MarkdownSection {
                source: source.to_string(),
            }],
        };
    }

    let mut sections = Vec::with_capacity(headings.len() + 1);
    let mut entries = Vec::with_capacity(headings.len());
    if let Some(first) = headings.first().map(|(offset, _, _)| *offset)
        && first > 0
    {
        sections.push(MarkdownSection {
            source: source[..first].to_string(),
        });
    }

    for (outline_index, (offset, level, text)) in headings.iter().enumerate() {
        let next = headings
            .get(outline_index + 1)
            .map(|(next_offset, _, _)| *next_offset)
            .unwrap_or(source.len());
        let section = sections.len();
        sections.push(MarkdownSection {
            source: source[*offset..next].to_string(),
        });
        entries.push(OutlineEntry {
            level: *level,
            text: text.clone(),
            section,
        });
    }

    ParsedDocument { entries, sections }
}

/// Returns the byte offset immediately after a leading YAML front matter block.
///
/// This mirrors the server's rendering rule: only a delimiter at the beginning
/// of the file with a matching `---` or `...` delimiter counts as metadata.
fn yaml_front_matter_end(source: &str) -> Option<usize> {
    let content = source.strip_prefix('\u{feff}').unwrap_or(source);
    let mut offset = source.len() - content.len();
    let mut lines = content.split_inclusive('\n');

    let first = lines.next()?;
    if first.trim_end_matches(['\r', '\n']) != "---" {
        return None;
    }
    offset += first.len();

    for line in lines {
        offset += line.len();
        match line.trim_end_matches(['\r', '\n']) {
            "---" | "..." => return Some(offset),
            _ => {}
        }
    }

    None
}

fn heading_text(nodes: &[Node]) -> String {
    let mut text = String::new();
    append_inline_text(nodes, &mut text);
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn append_inline_text(nodes: &[Node], output: &mut String) {
    for node in nodes {
        match node {
            Node::Text(value) => output.push_str(&value.value),
            Node::InlineCode(value) => output.push_str(&value.value),
            Node::Break(_) => output.push(' '),
            Node::Emphasis(value) => append_inline_text(&value.children, output),
            Node::Strong(value) => append_inline_text(&value.children, output),
            Node::Delete(value) => append_inline_text(&value.children, output),
            Node::Link(value) => append_inline_text(&value.children, output),
            Node::LinkReference(value) => append_inline_text(&value.children, output),
            Node::Image(value) => output.push_str(&value.alt),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_nested_headings_and_sections_in_document_order() {
        let parsed = parse_document("前言\n\n# 项目概览\n\n## 安装 `xwiki`\n\n### 下一步\n\n正文");

        assert_eq!(
            parsed.entries,
            vec![
                OutlineEntry {
                    level: 1,
                    text: "项目概览".into(),
                    section: 1,
                },
                OutlineEntry {
                    level: 2,
                    text: "安装 xwiki".into(),
                    section: 2,
                },
                OutlineEntry {
                    level: 3,
                    text: "下一步".into(),
                    section: 3,
                },
            ]
        );
        assert_eq!(parsed.sections.len(), 4);
        assert!(parsed.sections[0].source.contains("前言"));
    }

    #[test]
    fn supports_setext_headings_and_ignores_fenced_code() {
        let parsed = parse_document("标题\n===\n\n```md\n# 不是标题\n```\n\n## 真标题");

        assert_eq!(
            parsed
                .entries
                .iter()
                .map(|entry| entry.text.as_str())
                .collect::<Vec<_>>(),
            vec!["标题", "真标题"]
        );
    }

    #[test]
    fn excludes_leading_yaml_metadata_from_the_outline() {
        let parsed = parse_document(
            "---\ntitle: 资讯管理\nmodule: 资讯管理\nversion: v1.0\nsummary: 平台资讯的创建、查询、详情、修改\n---\n\n# 资讯管理\n\n## 创建资讯",
        );

        assert_eq!(
            parsed
                .entries
                .iter()
                .map(|entry| entry.text.as_str())
                .collect::<Vec<_>>(),
            vec!["资讯管理", "创建资讯"]
        );
        assert!(
            parsed.sections[0]
                .source
                .starts_with("---\ntitle: 资讯管理")
        );
    }

    #[test]
    fn documents_without_headings_still_have_one_renderable_section() {
        let parsed = parse_document("只有正文");
        assert!(parsed.entries.is_empty());
        assert_eq!(parsed.sections.len(), 1);
        assert_eq!(parsed.sections[0].source, "只有正文");
    }
}
