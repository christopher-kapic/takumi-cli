mod fonts;
mod html;
mod images;

use std::{
    borrow::Cow,
    fs,
    io::BufWriter,
    path::{Path, PathBuf},
};

use clap::Parser;
use takumi::{
    GlobalContext,
    layout::{Viewport, node::NodeKind},
    rendering::{ImageOutputFormat, RenderOptionsBuilder, render, write_image},
};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Parser)]
#[command(
    name = "takumi",
    about = "Render HTML or JSON nodes to images (supports Tailwind CSS via the `tw` prop)",
    after_long_help = "\
Examples:
  # Render inline HTML to an image
  takumi '<div style=\"display: flex; padding: 40px; background: #1e293b;\"><p style=\"color: white; font-size: 48px;\">Hello World</p></div>'

  # Render an HTML file
  takumi --file template.html -o card.png

  # Render a JSON node file
  takumi --file node.json -o card.png

  # Set custom dimensions and output format
  takumi '<div>Hello</div>' -W 800 -H 400 -o banner.webp

  # Use JPEG output with quality setting
  takumi '<div>Hello</div>' -o photo.jpg --quality 90

  # Images: use a remote URL in an <img> tag
  takumi '<img src=\"https://example.com/photo.jpg\" width=\"400\" height=\"300\" />'

  # Images: use a local file path in an <img> tag
  takumi '<img src=\"./logo.png\" width=\"200\" height=\"200\" />'

  # Fonts: load local font files with a glob pattern
  takumi --font './fonts/**/*.ttf' '<p style=\"font-family: MyFont;\">Custom font</p>'

  # Fonts: download and use a Google Font
  takumi --google-font 'Inter' '<p style=\"font-family: Inter;\">Google Font</p>'

  # Fonts: use multiple Google Fonts
  takumi --google-font 'Inter' --google-font 'Fira Code' '<p style=\"font-family: Inter;\">Mixed <span style=\"font-family: Fira Code;\">fonts</span></p>'

  # Tailwind CSS: use the `tw` prop instead of inline styles
  takumi '<div tw=\"flex p-10 bg-slate-800\"><p tw=\"text-white text-5xl\">Hello Tailwind!</p></div>'

  # Tailwind CSS: arbitrary values
  takumi '<div tw=\"flex bg-[#1e293b] p-[40px]\"><p tw=\"text-[48px] text-white font-bold\">Custom values</p></div>'

  # Combine features: custom size, Google Font, and remote image
  takumi -W 1200 -H 630 --google-font 'Poppins' '<div style=\"display: flex; align-items: center; gap: 20px; padding: 40px; background: linear-gradient(135deg, #667eea, #764ba2);\"><img src=\"https://example.com/avatar.jpg\" width=\"80\" height=\"80\" /><p style=\"color: white; font-family: Poppins; font-size: 36px;\">Open Graph Card</p></div>' -o og.png"
)]
struct Cli {
    /// Inline HTML string to render
    html: Option<String>,

    /// Path to a JSON node file (alternative to inline HTML)
    #[arg(short = 'f', long = "file")]
    file: Option<PathBuf>,

    /// Output file path
    #[arg(short = 'o', long = "output", default_value = "output.png")]
    output: PathBuf,

    /// Image width in pixels
    #[arg(short = 'W', long = "width", default_value_t = 1200)]
    width: u32,

    /// Image height in pixels
    #[arg(short = 'H', long = "height", default_value_t = 630)]
    height: u32,

    /// Output format: png, webp, jpeg (inferred from extension if omitted)
    #[arg(long = "format")]
    format: Option<String>,

    /// JPEG/WebP quality 0-100
    #[arg(long = "quality")]
    quality: Option<u8>,

    /// Draw debug borders
    #[arg(long = "debug")]
    debug: bool,

    /// Glob pattern to load font files
    #[arg(long = "font")]
    font: Option<String>,

    /// Google Font family name to download and use (can be repeated)
    #[arg(long = "google-font")]
    google_font: Vec<String>,
}

fn main() {
    let cli = Cli::parse();

    if cli.html.is_none() && cli.file.is_none() {
        eprintln!("Error: provide either an inline HTML string or --file <path>");
        std::process::exit(1);
    }

    // Determine output format
    let format = resolve_format(&cli.format, &cli.output);

    // Parse input into node tree
    let (node, stylesheets, base_dir) = parse_input(&cli);

    // Resolve images
    let fetched_resources = images::resolve_images(&node, &base_dir);

    // Set up global context and load fonts
    let mut global = GlobalContext::default();
    load_fonts(&mut global, cli.font.as_deref());
    for family in &cli.google_font {
        fonts::load_google_font(&mut global, family);
    }

    // Build render options and render
    let viewport = Viewport::new(Some(cli.width), Some(cli.height));

    let options = RenderOptionsBuilder::default()
        .viewport(viewport)
        .node(node)
        .global(&global)
        .draw_debug_border(cli.debug)
        .fetched_resources(fetched_resources)
        .stylesheets(stylesheets)
        .build()
        .expect("failed to build render options");

    let image = render(options).expect("render failed");

    // Write output
    let file = fs::File::create(&cli.output).expect("failed to create output file");
    let mut writer = BufWriter::new(file);

    write_image(Cow::Borrowed(&image), &mut writer, format, cli.quality)
        .expect("failed to write image");

    eprintln!(
        "Wrote {}x{} {} to {}",
        image.width(),
        image.height(),
        format_name(format),
        cli.output.display()
    );
}

fn parse_input(cli: &Cli) -> (NodeKind, Vec<String>, PathBuf) {
    if let Some(ref file_path) = cli.file {
        let content = fs::read_to_string(file_path).expect("failed to read input file");
        let base_dir = file_path
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();

        // Try JSON first
        if file_path
            .extension()
            .is_some_and(|ext| ext == "json")
        {
            let node: NodeKind =
                serde_json::from_str(&content).expect("failed to parse JSON node file");
            return (node, Vec::new(), base_dir);
        }

        // Otherwise treat as HTML
        let (node, stylesheets) = html::parse_html(&content);
        (node, stylesheets, base_dir)
    } else {
        let html_str = cli.html.as_deref().unwrap();
        let base_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let (node, stylesheets) = html::parse_html(html_str);
        (node, stylesheets, base_dir)
    }
}

fn resolve_format(explicit: &Option<String>, output: &Path) -> ImageOutputFormat {
    if let Some(fmt) = explicit {
        match fmt.to_lowercase().as_str() {
            "png" => return ImageOutputFormat::Png,
            "webp" => return ImageOutputFormat::WebP,
            "jpeg" | "jpg" => return ImageOutputFormat::Jpeg,
            other => {
                eprintln!("Unknown format '{other}', defaulting to PNG");
                return ImageOutputFormat::Png;
            }
        }
    }

    // Infer from extension
    match output
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("webp") => ImageOutputFormat::WebP,
        Some("jpg" | "jpeg") => ImageOutputFormat::Jpeg,
        _ => ImageOutputFormat::Png,
    }
}

fn format_name(format: ImageOutputFormat) -> &'static str {
    match format {
        ImageOutputFormat::Png => "PNG",
        ImageOutputFormat::WebP => "WebP",
        ImageOutputFormat::Jpeg => "JPEG",
    }
}

fn load_fonts(global: &mut GlobalContext, pattern: Option<&str>) {
    // Load bundled font
    global
        .font_context
        .load_and_store(
            Cow::Borrowed(include_bytes!("../../assets/fonts/geist/Geist[wght].woff2")),
            None,
            None,
        )
        .ok();

    // Load fonts matching glob pattern
    if let Some(pattern) = pattern {
        let entries = glob::glob(pattern).expect("invalid font glob pattern");
        for entry in entries.flatten() {
            let bytes = match fs::read(&entry) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Warning: failed to read font {}: {e}", entry.display());
                    continue;
                }
            };
            if let Err(e) = global
                .font_context
                .load_and_store(Cow::Owned(bytes), None, None)
            {
                eprintln!("Warning: failed to load font {}: {e}", entry.display());
            }
        }
    }
}
