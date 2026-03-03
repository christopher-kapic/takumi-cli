use std::{
    borrow::Cow,
    fs,
    path::PathBuf,
};

use takumi::GlobalContext;

/// Returns the font cache directory:
/// - Linux: `~/.local/share/takumi/fonts/`
/// - macOS: `~/Library/Application Support/takumi/fonts/`
fn cache_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("takumi").join("fonts"))
}

/// Load a Google Font by family name.
/// Downloads and caches the font files if not already present.
pub fn load_google_font(global: &mut GlobalContext, family: &str) {
    let Some(cache) = cache_dir() else {
        eprintln!("Warning: could not determine data directory, skipping Google Font '{family}'");
        return;
    };

    // Check cache first — look for any files matching this family
    let family_dir = cache.join(sanitize_name(family));
    if family_dir.is_dir() {
        let mut loaded = false;
        if let Ok(entries) = fs::read_dir(&family_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if is_font_file(&path) {
                    if let Ok(bytes) = fs::read(&path) {
                        if global
                            .font_context
                            .load_and_store(Cow::Owned(bytes), None, None)
                            .is_ok()
                        {
                            loaded = true;
                        }
                    }
                }
            }
        }
        if loaded {
            eprintln!("Loaded cached Google Font '{family}'");
            return;
        }
    }

    // Fetch from Google Fonts CSS API
    eprintln!("Downloading Google Font '{family}'...");
    let css_url = format!(
        "https://fonts.googleapis.com/css2?family={}&display=swap",
        family.replace(' ', "+")
    );

    // Request with a user-agent that triggers woff2 URLs
    let css = match ureq::get(&css_url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/120",
        )
        .call()
    {
        Ok(resp) => match resp.into_body().read_to_string() {
            Ok(body) => body,
            Err(e) => {
                eprintln!("Warning: failed to read Google Fonts CSS for '{family}': {e}");
                return;
            }
        },
        Err(e) => {
            eprintln!("Warning: failed to fetch Google Fonts CSS for '{family}': {e}");
            return;
        }
    };

    // Extract font URLs from the CSS
    let urls = extract_font_urls(&css);
    if urls.is_empty() {
        eprintln!("Warning: no font files found for '{family}' — check the font name");
        return;
    }

    // Create cache directory
    if let Err(e) = fs::create_dir_all(&family_dir) {
        eprintln!("Warning: failed to create cache dir {}: {e}", family_dir.display());
        return;
    }

    let mut loaded = 0;
    for (i, url) in urls.iter().enumerate() {
        let ext = if url.contains(".woff2") {
            "woff2"
        } else if url.contains(".woff") {
            "woff"
        } else {
            "ttf"
        };
        let filename = format!("{i}.{ext}");
        let file_path = family_dir.join(&filename);

        let bytes = match fetch_bytes(url) {
            Some(b) => b,
            None => continue,
        };

        // Save to cache
        if let Err(e) = fs::write(&file_path, &bytes) {
            eprintln!("Warning: failed to cache font to {}: {e}", file_path.display());
        }

        // Load into context
        if global
            .font_context
            .load_and_store(Cow::Owned(bytes), None, None)
            .is_ok()
        {
            loaded += 1;
        }
    }

    if loaded > 0 {
        eprintln!("Downloaded and cached {loaded} font file(s) for '{family}'");
    } else {
        eprintln!("Warning: failed to load any font files for '{family}'");
    }
}

/// Extract `url(...)` values from Google Fonts CSS.
fn extract_font_urls(css: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for chunk in css.split("url(") {
        if let Some(end) = chunk.find(')') {
            let url = chunk[..end].trim().trim_matches(|c| c == '\'' || c == '"');
            if url.starts_with("https://") {
                urls.push(url.to_string());
            }
        }
    }
    urls
}

/// Fetch raw bytes from a URL.
fn fetch_bytes(url: &str) -> Option<Vec<u8>> {
    let resp = ureq::get(url)
        .call()
        .map_err(|e| eprintln!("Warning: failed to fetch {url}: {e}"))
        .ok()?;

    resp.into_body()
        .read_to_vec()
        .map_err(|e| eprintln!("Warning: failed to read body from {url}: {e}"))
        .ok()
}

/// Sanitize a font family name for use as a directory name.
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect()
}

fn is_font_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ttf" | "otf" | "woff" | "woff2")
    )
}
