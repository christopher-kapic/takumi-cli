use std::str::FromStr;
use std::sync::Arc;

use takumi::layout::{
    node::{ContainerNode, ImageNode, NodeKind, TextNode},
    style::tw::TailwindValues,
};

/// Parse an HTML string into a `NodeKind` tree and collected CSS stylesheets.
///
/// For `style="..."` attributes, generates synthetic class names and CSS rules.
/// For `tw="..."` attributes, passes through to the node's `tw` field.
/// For `class="..."` attributes, sets `class_name` on the node.
pub fn parse_html(html: &str) -> (NodeKind, Vec<String>) {
    let mut stylesheets = Vec::new();
    let mut class_counter = 0usize;

    let dom = tl::parse(html, tl::ParserOptions::default()).expect("failed to parse HTML");
    let parser = dom.parser();

    let top_level: Vec<_> = dom.children().iter().copied().collect();

    let children: Vec<NodeKind> = top_level
        .iter()
        .filter_map(|handle| convert_node(*handle, parser, &mut stylesheets, &mut class_counter))
        .collect();

    let root = if children.len() == 1 {
        children.into_iter().next().unwrap()
    } else {
        NodeKind::Container(ContainerNode {
            children: Some(children.into_boxed_slice()),
            tag_name: None,
            class_name: None,
            id: None,
            preset: None,
            style: None,
            tw: None,
        })
    };

    (root, stylesheets)
}

fn convert_node(
    handle: tl::NodeHandle,
    parser: &tl::Parser,
    stylesheets: &mut Vec<String>,
    class_counter: &mut usize,
) -> Option<NodeKind> {
    let node = handle.get(parser)?;

    match node {
        tl::Node::Tag(tag) => {
            let tag_name = tag.name().as_utf8_str().to_lowercase();

            // Extract attributes
            let attrs = tag.attributes();
            let style_attr = attrs
                .get("style")
                .flatten()
                .map(|v| v.as_utf8_str().to_string());
            let tw_attr = attrs
                .get("tw")
                .flatten()
                .map(|v| v.as_utf8_str().to_string());
            let class_attr = attrs
                .get("class")
                .flatten()
                .map(|v| v.as_utf8_str().to_string());
            let id_attr = attrs
                .get("id")
                .flatten()
                .map(|v| v.as_utf8_str().to_string());

            // Handle style → synthetic class name + CSS rule
            let mut generated_class = None;
            if let Some(ref style_str) = style_attr {
                if !style_str.is_empty() {
                    let cls = format!("_s{}", *class_counter);
                    *class_counter += 1;
                    stylesheets.push(format!(".{cls} {{ {style_str} }}"));
                    generated_class = Some(cls);
                }
            }

            // Merge class_attr + generated_class
            let final_class: Option<Box<str>> = match (class_attr, generated_class) {
                (Some(user), Some(generated)) => Some(format!("{user} {generated}").into()),
                (Some(user), None) => Some(user.into()),
                (None, Some(generated)) => Some(generated.into()),
                (None, None) => None,
            };

            let tw: Option<TailwindValues> = tw_attr.and_then(|s| TailwindValues::from_str(&s).ok());
            let id: Option<Box<str>> = id_attr.map(Into::into);

            if tag_name == "img" {
                let src = attrs
                    .get("src")
                    .flatten()
                    .map(|v| v.as_utf8_str().to_string())
                    .unwrap_or_default();
                let width = attrs
                    .get("width")
                    .flatten()
                    .and_then(|v| v.as_utf8_str().parse::<f32>().ok());
                let height = attrs
                    .get("height")
                    .flatten()
                    .and_then(|v| v.as_utf8_str().parse::<f32>().ok());

                return Some(NodeKind::Image(ImageNode {
                    tag_name: Some("img".into()),
                    class_name: final_class,
                    id,
                    src: Arc::from(src.as_str()),
                    width,
                    height,
                    tw,
                    ..Default::default()
                }));
            }

            // Convert children
            let child_nodes = tag.children();
            let child_handles: Vec<_> = child_nodes.top().iter().copied().collect();
            let children: Vec<NodeKind> = child_handles
                .iter()
                .filter_map(|h| convert_node(*h, parser, stylesheets, class_counter))
                .collect();

            // Text-only children get a TextNode
            if children.is_empty() {
                let inner_text = collect_text_content(tag, parser);
                if !inner_text.is_empty() {
                    return Some(NodeKind::Text(TextNode {
                        tag_name: Some(tag_name.into()),
                        class_name: final_class,
                        id,
                        text: inner_text,
                        tw,
                        ..Default::default()
                    }));
                }
            }

            Some(NodeKind::Container(ContainerNode {
                tag_name: Some(tag_name.into()),
                class_name: final_class,
                id,
                children: if children.is_empty() {
                    None
                } else {
                    Some(children.into_boxed_slice())
                },
                tw,
                preset: None,
                style: None,
            }))
        }
        tl::Node::Raw(raw) => {
            let text = raw.as_utf8_str().trim().to_string();
            if text.is_empty() {
                return None;
            }
            Some(NodeKind::Text(TextNode {
                text,
                ..Default::default()
            }))
        }
        _ => None,
    }
}

/// Collect direct text content from a tag's children (non-recursive for simple cases).
fn collect_text_content(tag: &tl::HTMLTag, parser: &tl::Parser) -> String {
    let mut text = String::new();
    for handle in tag.children().top().iter() {
        if let Some(tl::Node::Raw(raw)) = handle.get(parser) {
            let s = raw.as_utf8_str();
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(trimmed);
            }
        }
    }
    text
}
