use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::app::RenderMode;
use crate::html::{collapse_whitespace, Document, NodeId, NodeKind};

#[derive(Debug, Clone)]
pub struct LinkRegion {
    pub line_index: usize,
    pub col_start: usize,
    pub col_end: usize,
    pub url: String,
    pub label: String,
}

#[derive(Debug, Clone, Default)]
pub struct RenderedPage {
    pub lines: Vec<Line<'static>>,
    pub links: Vec<LinkRegion>,
}

#[derive(Debug, Clone)]
struct StyledRun {
    text: String,
    style: Style,
    href: Option<String>,
    preformatted: bool,
}

#[derive(Debug, Clone, Copy)]
struct TraversalContext {
    bold: bool,
    italic: bool,
    underline: bool,
    code: bool,
    preformatted: bool,
    heading_level: u8,
    quote_depth: usize,
}

pub fn render_document(
    doc: &Document,
    width: u16,
    mode: RenderMode,
    focused_link: Option<usize>,
) -> RenderedPage {
    let max_width = (width as usize).max(20);
    let mut items: Vec<FlowItem> = Vec::new();
    let ctx = TraversalContext {
        bold: false,
        italic: false,
        underline: false,
        code: false,
        preformatted: false,
        heading_level: 0,
        quote_depth: 0,
    };

    collect_flow(doc, doc.root, None, ctx, mode, &mut items);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut links: Vec<LinkRegion> = Vec::new();

    for item in items {
        match item {
            FlowItem::BlankLine => {
                if !lines.last().map(|l| l.spans.is_empty()).unwrap_or(true) {
                    lines.push(Line::default());
                }
            }
            FlowItem::HorizontalRule => {
                let rule_char = match mode {
                    RenderMode::Text => "-",
                    RenderMode::Color | RenderMode::Pixel => "─",
                };
                let style = match mode {
                    RenderMode::Text => Style::default(),
                    RenderMode::Color | RenderMode::Pixel => Style::default().fg(Color::DarkGray),
                };
                lines.push(Line::from(Span::styled(
                    rule_char.repeat(max_width),
                    style,
                )));
            }
            FlowItem::Paragraph { prefix, runs } => {
                wrap_runs_into_lines(
                    &prefix,
                    &runs,
                    max_width,
                    mode,
                    focused_link,
                    &mut lines,
                    &mut links,
                );
            }
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "(Empty document)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    RenderedPage { lines, links }
}

enum FlowItem {
    BlankLine,
    HorizontalRule,
    Paragraph {
        prefix: String,
        runs: Vec<StyledRun>,
    },
}

fn collect_flow(
    doc: &Document,
    node_id: NodeId,
    active_href: Option<&str>,
    mut ctx: TraversalContext,
    mode: RenderMode,
    out: &mut Vec<FlowItem>,
) {
    let node = &doc.nodes[node_id];
    match &node.kind {
        NodeKind::Document => {
            for &child in &node.children {
                collect_flow(doc, child, active_href, ctx, mode, out);
            }
        }
        NodeKind::Text(text) => {
            let normalized = if ctx.preformatted {
                text.clone()
            } else {
                collapse_whitespace(text)
            };
            if normalized.trim().is_empty() && !ctx.preformatted {
                return;
            }
            let style = build_style(ctx, active_href.is_some(), mode);
            push_run(
                out,
                StyledRun {
                    text: normalized,
                    style,
                    href: active_href.map(ToString::to_string),
                    preformatted: ctx.preformatted,
                },
            );
        }
        NodeKind::Element { tag, attrs } => {
            if matches!(
                tag.as_str(),
                "head" | "script" | "style" | "noscript" | "template" | "meta" | "link"
            ) {
                return;
            }

            if tag == "hr" {
                out.push(FlowItem::BlankLine);
                out.push(FlowItem::HorizontalRule);
                out.push(FlowItem::BlankLine);
                return;
            }

            if tag == "br" {
                out.push(FlowItem::Paragraph {
                    prefix: String::new(),
                    runs: Vec::new(),
                });
                return;
            }

            if tag == "img" {
                let alt = attrs
                    .get("alt")
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("image");
                let label = match mode {
                    RenderMode::Text => format!("[IMG: {}]", alt),
                    RenderMode::Color => format!("🖼 [IMG: {}]", alt),
                    RenderMode::Pixel => format!("▣ [PIXEL IMAGE: {}]", alt),
                };
                let style = match mode {
                    RenderMode::Text => Style::default(),
                    RenderMode::Color => Style::default().fg(Color::Magenta),
                    RenderMode::Pixel => Style::default()
                        .fg(Color::LightMagenta)
                        .add_modifier(Modifier::BOLD),
                };
                push_run(
                    out,
                    StyledRun {
                        text: label,
                        style,
                        href: active_href.map(ToString::to_string),
                        preformatted: false,
                    },
                );
                return;
            }

            let resolved_href = if tag == "a" {
                attrs
                    .get("href")
                    .and_then(|h| doc.resolve_href(h))
                    .or_else(|| active_href.map(ToString::to_string))
            } else {
                active_href.map(ToString::to_string)
            };

            let is_block = matches!(
                tag.as_str(),
                "p" | "div"
                    | "section"
                    | "article"
                    | "header"
                    | "footer"
                    | "main"
                    | "nav"
                    | "ul"
                    | "ol"
                    | "li"
                    | "h1"
                    | "h2"
                    | "h3"
                    | "h4"
                    | "h5"
                    | "h6"
                    | "blockquote"
                    | "pre"
                    | "tr"
            );

            match tag.as_str() {
                "h1" => {
                    ctx.heading_level = 1;
                    ctx.bold = true;
                }
                "h2" => {
                    ctx.heading_level = 2;
                    ctx.bold = true;
                }
                "h3" | "h4" | "h5" | "h6" => {
                    ctx.heading_level = 3;
                    ctx.bold = true;
                }
                "strong" | "b" => ctx.bold = true,
                "em" | "i" => ctx.italic = true,
                "u" => ctx.underline = true,
                "code" => ctx.code = true,
                "pre" => {
                    ctx.code = true;
                    ctx.preformatted = true;
                }
                "blockquote" => {
                    ctx.quote_depth += 1;
                    ctx.italic = true;
                }
                _ => {}
            }

            if is_block {
                out.push(FlowItem::BlankLine);
                let mut prefix = "│ ".repeat(ctx.quote_depth);
                if tag == "li" {
                    prefix.push_str("• ");
                } else if tag == "h1" {
                    prefix.push_str("# ");
                } else if tag == "h2" {
                    prefix.push_str("## ");
                } else if matches!(tag.as_str(), "h3" | "h4" | "h5" | "h6") {
                    prefix.push_str("### ");
                }
                out.push(FlowItem::Paragraph {
                    prefix,
                    runs: Vec::new(),
                });
            }

            for &child in &node.children {
                collect_flow(doc, child, resolved_href.as_deref(), ctx, mode, out);
            }

            if is_block {
                out.push(FlowItem::BlankLine);
            }
        }
    }
}

fn push_run(out: &mut Vec<FlowItem>, run: StyledRun) {
    if !matches!(out.last(), Some(FlowItem::Paragraph { .. })) {
        out.push(FlowItem::Paragraph {
            prefix: String::new(),
            runs: Vec::new(),
        });
    }
    if let Some(FlowItem::Paragraph { runs, .. }) = out.last_mut() {
        runs.push(run);
    }
}

fn build_style(ctx: TraversalContext, is_link: bool, mode: RenderMode) -> Style {
    let mut style = Style::default();
    if ctx.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if ctx.italic {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if ctx.underline || is_link {
        style = style.add_modifier(Modifier::UNDERLINED);
    }

    if mode == RenderMode::Text {
        return style;
    }

    if is_link {
        style = style.fg(Color::Cyan);
    } else if ctx.heading_level == 1 {
        style = style.fg(Color::Yellow);
    } else if ctx.heading_level == 2 {
        style = style.fg(Color::Green);
    } else if ctx.heading_level >= 3 {
        style = style.fg(Color::LightBlue);
    } else if ctx.code {
        style = style.fg(Color::LightYellow);
    } else if ctx.quote_depth > 0 {
        style = style.fg(Color::Gray);
    }

    style
}

fn wrap_runs_into_lines(
    prefix: &str,
    runs: &[StyledRun],
    max_width: usize,
    mode: RenderMode,
    focused_link: Option<usize>,
    lines: &mut Vec<Line<'static>>,
    links: &mut Vec<LinkRegion>,
) {
    if runs.is_empty() && prefix.is_empty() {
        return;
    }

    let prefix_len = prefix.chars().count();
    let indent = " ".repeat(prefix_len);
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut current_col = 0usize;

    if !prefix.is_empty() {
        let prefix_style = if mode == RenderMode::Text {
            Style::default()
        } else {
            Style::default().fg(Color::DarkGray)
        };
        current_spans.push(Span::styled(prefix.to_string(), prefix_style));
        current_col = prefix_len;
    }

    let mut active_link_index: Option<usize> = None;
    let mut last_href: Option<String> = None;

    for run in runs {
        if run.href != last_href {
            active_link_index = None;
            last_href = run.href.clone();
        }

        if run.preformatted {
            for raw_line in run.text.split('\n') {
                let line_idx = lines.len();
                let chars_len = raw_line.chars().count();
                let mut span_style = run.style;

                if let Some(href) = &run.href {
                    let link_idx = match active_link_index {
                        Some(idx) => idx,
                        None => {
                            let idx = links.len();
                            links.push(LinkRegion {
                                line_index: line_idx,
                                col_start: current_col,
                                col_end: current_col + chars_len,
                                url: href.clone(),
                                label: raw_line.trim().to_string(),
                            });
                            active_link_index = Some(idx);
                            idx
                        }
                    };
                    if focused_link == Some(link_idx) {
                        span_style = focused_link_style(mode);
                    }
                }

                current_spans.push(Span::styled(raw_line.to_string(), span_style));
                lines.push(Line::from(std::mem::take(&mut current_spans)));
                current_col = 0;
            }
            continue;
        }

        for word in run.text.split_inclusive(' ') {
            let word_len = word.chars().count();
            if current_col > prefix_len && current_col + word_len > max_width {
                lines.push(Line::from(std::mem::take(&mut current_spans)));
                if !indent.is_empty() {
                    current_spans.push(Span::raw(indent.clone()));
                }
                current_col = prefix_len;
            }

            let trimmed_word = if current_col == prefix_len {
                word.trim_start()
            } else {
                word
            };
            let placed_len = trimmed_word.chars().count();
            if placed_len == 0 {
                continue;
            }

            let line_idx = lines.len();
            let mut span_style = run.style;

            if let Some(href) = &run.href {
                let link_idx = match active_link_index {
                    Some(idx) => {
                        if links[idx].line_index == line_idx {
                            links[idx].col_end = current_col + placed_len;
                            if !links[idx].label.is_empty() {
                                links[idx].label.push(' ');
                            }
                            links[idx].label.push_str(trimmed_word.trim());
                        }
                        idx
                    }
                    None => {
                        let idx = links.len();
                        links.push(LinkRegion {
                            line_index: line_idx,
                            col_start: current_col,
                            col_end: current_col + placed_len,
                            url: href.clone(),
                            label: trimmed_word.trim().to_string(),
                        });
                        active_link_index = Some(idx);
                        idx
                    }
                };
                if focused_link == Some(link_idx) {
                    span_style = focused_link_style(mode);
                }
            }

            current_spans.push(Span::styled(trimmed_word.to_string(), span_style));
            current_col += placed_len;
        }
    }

    if !current_spans.is_empty() {
        lines.push(Line::from(current_spans));
    }
}

fn focused_link_style(mode: RenderMode) -> Style {
    match mode {
        RenderMode::Text => Style::default().add_modifier(Modifier::REVERSED),
        RenderMode::Color | RenderMode::Pixel => Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    }
}
