use std::collections::HashMap;
use url::Url;

pub type NodeId = usize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    Document,
    Element {
        tag: String,
        attrs: HashMap<String, String>,
    },
    Text(String),
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub kind: NodeKind,
}

#[derive(Debug, Clone)]
pub struct Document {
    pub nodes: Vec<Node>,
    pub root: NodeId,
    pub title: String,
    pub base_url: String,
}

impl Default for Document {
    fn default() -> Self {
        Self::welcome()
    }
}

impl Document {
    pub fn welcome() -> Self {
        let html = r#"<!doctype html>
<html>
<head><title>OrbitPixel Start</title></head>
<body>
  <h1>OrbitPixel Browser</h1>
  <p>Terminal-native web browser powered by Rust. Press <strong>Ctrl+L</strong> or <strong>i</strong> to enter a URL.</p>
  <h2>Quick Links</h2>
  <ul>
    <li><a href="https://example.com">Example Domain (https://example.com)</a></li>
    <li><a href="https://news.ycombinator.com">Hacker News (https://news.ycombinator.com)</a></li>
    <li><a href="https://lite.cnn.com">CNN Lite (https://lite.cnn.com)</a></li>
    <li><a href="https://www.rust-lang.org">Rust Programming Language</a></li>
  </ul>
</body>
</html>"#;
        Self::parse(html, "https://orbitpixel.local/")
    }

    pub fn error_page(url: &str, message: &str) -> Self {
        let escaped_url = escape_html(url);
        let escaped_msg = escape_html(message);
        let html = format!(
            r#"<!doctype html>
<html>
<head><title>Navigation Error</title></head>
<body>
  <h1>Unable to Load Page</h1>
  <p>Target URL: <code>{escaped_url}</code></p>
  <blockquote>{escaped_msg}</blockquote>
  <p>Press <strong>Ctrl+L</strong> to enter another URL or <strong>Ctrl+R</strong> to retry.</p>
</body>
</html>"#
        );
        Self::parse(&html, url)
    }

    pub fn parse(html: &str, base_url: &str) -> Self {
        let mut nodes = vec![Node {
            id: 0,
            parent: None,
            children: Vec::new(),
            kind: NodeKind::Document,
        }];
        let mut stack: Vec<NodeId> = vec![0];
        let mut title = String::new();
        let mut resolved_base = base_url.to_string();

        for token in tokenize_html(html) {
            match token {
                HtmlToken::StartTag {
                    tag,
                    attrs,
                    self_closing,
                } => {
                    if tag == "base" {
                        if let Some(href) = attrs.get("href") {
                            if let Ok(base) = Url::parse(base_url) {
                                if let Ok(joined) = base.join(href) {
                                    resolved_base = joined.to_string();
                                }
                            }
                        }
                    }

                    let parent_id = *stack.last().unwrap_or(&0);
                    let id = nodes.len();
                    nodes.push(Node {
                        id,
                        parent: Some(parent_id),
                        children: Vec::new(),
                        kind: NodeKind::Element {
                            tag: tag.clone(),
                            attrs,
                        },
                    });
                    nodes[parent_id].children.push(id);

                    if !self_closing && !is_void_element(&tag) {
                        stack.push(id);
                    }
                }
                HtmlToken::EndTag { tag } => {
                    if let Some(pos) = stack.iter().rposition(|&nid| match &nodes[nid].kind {
                        NodeKind::Element { tag: open_tag, .. } => open_tag == &tag,
                        _ => false,
                    }) {
                        if pos > 0 {
                            stack.truncate(pos);
                        }
                    }
                }
                HtmlToken::Text(raw_text) => {
                    let decoded = decode_entities(&raw_text);
                    if decoded.is_empty() {
                        continue;
                    }
                    let parent_id = *stack.last().unwrap_or(&0);
                    if let NodeKind::Element { tag, .. } = &nodes[parent_id].kind {
                        if tag == "title" {
                            title.push_str(decoded.trim());
                        }
                    }
                    let id = nodes.len();
                    nodes.push(Node {
                        id,
                        parent: Some(parent_id),
                        children: Vec::new(),
                        kind: NodeKind::Text(decoded),
                    });
                    nodes[parent_id].children.push(id);
                }
            }
        }

        let clean_title = if title.trim().is_empty() {
            base_url.to_string()
        } else {
            collapse_whitespace(&title)
        };

        Self {
            nodes,
            root: 0,
            title: clean_title,
            base_url: resolved_base,
        }
    }

    pub fn resolve_href(&self, href: &str) -> Option<String> {
        let trimmed = href.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("javascript:") {
            return None;
        }
        let base = Url::parse(&self.base_url).ok()?;
        let joined = base.join(trimmed).ok()?;
        match joined.scheme() {
            "http" | "https" => Some(joined.to_string()),
            _ => None,
        }
    }
}

#[derive(Debug)]
enum HtmlToken {
    StartTag {
        tag: String,
        attrs: HashMap<String, String>,
        self_closing: bool,
    },
    EndTag {
        tag: String,
    },
    Text(String),
}

fn tokenize_html(input: &str) -> Vec<HtmlToken> {
    let mut tokens = Vec::new();
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'<' {
            if input[i..].starts_with("<!--") {
                if let Some(end) = input[i + 4..].find("-->") {
                    i += 4 + end + 3;
                } else {
                    break;
                }
                continue;
            }
            if input[i..].starts_with("<!") || input[i..].starts_with("<?") {
                if let Some(end) = input[i..].find('>') {
                    i += end + 1;
                } else {
                    break;
                }
                continue;
            }

            if let Some(gt_offset) = find_tag_end(&input[i..]) {
                let raw_tag = &input[i + 1..i + gt_offset];
                i += gt_offset + 1;

                let trimmed = raw_tag.trim();
                if trimmed.is_empty() {
                    continue;
                }

                if let Some(rest) = trimmed.strip_prefix('/') {
                    let tag_name = rest
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    if !tag_name.is_empty() {
                        tokens.push(HtmlToken::EndTag { tag: tag_name });
                    }
                } else {
                    let self_closing = trimmed.ends_with('/');
                    let inner = if self_closing {
                        trimmed[..trimmed.len() - 1].trim()
                    } else {
                        trimmed
                    };
                    let (tag_name, attrs) = parse_tag_and_attrs(inner);
                    if !tag_name.is_empty() {
                        let is_raw_text_tag = matches!(tag_name.as_str(), "script" | "style");
                        tokens.push(HtmlToken::StartTag {
                            tag: tag_name.clone(),
                            attrs,
                            self_closing,
                        });
                        if is_raw_text_tag && !self_closing {
                            let close_needle = format!("</{}", tag_name);
                            let lower_tail = input[i..].to_ascii_lowercase();
                            if let Some(close_pos) = lower_tail.find(&close_needle) {
                                i += close_pos;
                            } else {
                                i = len;
                            }
                        }
                    }
                }
                continue;
            }
        }

        let next_lt = input[i..].find('<').map(|offset| i + offset).unwrap_or(len);
        if next_lt > i {
            tokens.push(HtmlToken::Text(input[i..next_lt].to_string()));
            i = next_lt;
        } else {
            tokens.push(HtmlToken::Text(input[i..i + 1].to_string()));
            i += 1;
        }
    }

    tokens
}

fn find_tag_end(slice: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (idx, ch) in slice.char_indices() {
        match quote {
            Some(q) if ch == q => quote = None,
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch == '>' => return Some(idx),
            _ => {}
        }
    }
    None
}

fn parse_tag_and_attrs(inner: &str) -> (String, HashMap<String, String>) {
    let mut attrs = HashMap::new();
    let mut chars = inner.char_indices().peekable();

    let mut tag_end = inner.len();
    while let Some(&(idx, ch)) = chars.peek() {
        if ch.is_ascii_whitespace() {
            tag_end = idx;
            break;
        }
        chars.next();
    }
    let tag = inner[..tag_end].to_ascii_lowercase();

    while let Some(&(idx, ch)) = chars.peek() {
        if ch.is_ascii_whitespace() {
            chars.next();
            continue;
        }
        let name_start = idx;
        while let Some(&(_, c)) = chars.peek() {
            if c.is_ascii_whitespace() || c == '=' {
                break;
            }
            chars.next();
        }
        let name_end = chars.peek().map(|&(p, _)| p).unwrap_or(inner.len());
        let attr_name = inner[name_start..name_end].to_ascii_lowercase();

        while let Some(&(_, c)) = chars.peek() {
            if c.is_ascii_whitespace() {
                chars.next();
            } else {
                break;
            }
        }

        if let Some(&(_, '=')) = chars.peek() {
            chars.next();
            while let Some(&(_, c)) = chars.peek() {
                if c.is_ascii_whitespace() {
                    chars.next();
                } else {
                    break;
                }
            }
            let val = if let Some(&(q_idx, q @ ('"' | '\''))) = chars.peek() {
                chars.next();
                let val_start = q_idx + q.len_utf8();
                let mut val_end = inner.len();
                while let Some(&(c_idx, c)) = chars.peek() {
                    chars.next();
                    if c == q {
                        val_end = c_idx;
                        break;
                    }
                }
                inner[val_start..val_end].to_string()
            } else {
                let val_start = chars.peek().map(|&(p, _)| p).unwrap_or(inner.len());
                while let Some(&(_, c)) = chars.peek() {
                    if c.is_ascii_whitespace() {
                        break;
                    }
                    chars.next();
                }
                let val_end = chars.peek().map(|&(p, _)| p).unwrap_or(inner.len());
                inner[val_start..val_end].to_string()
            };
            if !attr_name.is_empty() {
                attrs.insert(attr_name, decode_entities(&val));
            }
        } else if !attr_name.is_empty() {
            attrs.insert(attr_name, String::new());
        }
    }

    (tag, attrs)
}

fn is_void_element(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

pub fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
