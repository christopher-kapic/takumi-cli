use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use takumi::{
    layout::node::NodeKind,
    resources::image::{ImageSource, load_image_source_from_bytes},
};

/// Walk the node tree and collect all image sources that need fetching.
/// Returns a map of source string → loaded ImageSource.
pub fn resolve_images(
    node: &NodeKind,
    base_dir: &Path,
) -> HashMap<Arc<str>, Arc<ImageSource>> {
    let mut sources: Vec<Arc<str>> = Vec::new();
    collect_image_sources(node, &mut sources);

    let mut result = HashMap::new();

    for src in sources {
        if result.contains_key(&src) {
            continue;
        }

        if let Some(image) = resolve_single_source(&src, base_dir) {
            result.insert(src, image);
        }
    }

    result
}

fn collect_image_sources(node: &NodeKind, sources: &mut Vec<Arc<str>>) {
    match node {
        NodeKind::Image(img) => {
            sources.push(img.src.clone());
        }
        NodeKind::Container(container) => {
            if let Some(children) = container.children.as_deref() {
                for child in children {
                    collect_image_sources(child, sources);
                }
            }
        }
        NodeKind::Text(_) => {}
    }
}

fn resolve_single_source(src: &str, base_dir: &Path) -> Option<Arc<ImageSource>> {
    // Skip data URIs and inline SVG — handled by the renderer
    if src.starts_with("data:") || is_inline_svg(src) {
        return None;
    }

    // HTTP(S) URLs — fetch remotely
    if src.starts_with("http://") || src.starts_with("https://") {
        return fetch_remote(src);
    }

    // Local file path
    let path = resolve_local_path(src, base_dir);
    load_local_file(&path)
}

fn is_inline_svg(src: &str) -> bool {
    src.contains("<svg") && src.contains("xmlns")
}

fn resolve_local_path(src: &str, base_dir: &Path) -> PathBuf {
    let path = Path::new(src);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

fn load_local_file(path: &Path) -> Option<Arc<ImageSource>> {
    let bytes = std::fs::read(path)
        .map_err(|e| eprintln!("Warning: failed to read {}: {e}", path.display()))
        .ok()?;

    load_image_source_from_bytes(&bytes)
        .map_err(|e| eprintln!("Warning: failed to decode {}: {e}", path.display()))
        .ok()
}

fn fetch_remote(url: &str) -> Option<Arc<ImageSource>> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| eprintln!("Warning: failed to fetch {url}: {e}"))
        .ok()?;

    let bytes = response
        .into_body()
        .read_to_vec()
        .map_err(|e| eprintln!("Warning: failed to read response body from {url}: {e}"))
        .ok()?;

    load_image_source_from_bytes(&bytes)
        .map_err(|e| eprintln!("Warning: failed to decode image from {url}: {e}"))
        .ok()
}
