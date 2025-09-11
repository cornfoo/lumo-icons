use convert_case::{Case, Casing};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use regex::Regex;
use std::collections::{BTreeMap, HashMap};
use std::{fs, process};

fn extract_categories(input: &str) -> (HashMap<String, Vec<String>>, BTreeMap<String, ()>) {
    let mut icon_categories: HashMap<String, Vec<String>> = HashMap::new();
    let mut categories_set: BTreeMap<String, ()> = BTreeMap::new();

    let parsed: serde_json::Value = serde_json::from_str(input).unwrap();
    if let serde_json::Value::Object(map) = parsed {
        for (icon_name, data) in map {
            if let Some(sets) = data.get("sets").and_then(|s| s.as_array()) {
                let mut list = Vec::new();
                for s in sets {
                    if let Some(set_str) = s.as_str() {
                        categories_set.insert(set_str.to_string(), ());
                        list.push(set_str.to_string());
                    }
                }
                icon_categories.insert(icon_name, list);
            }
        }
    }

    categories_set.insert("uncategorized".to_string(), ());
    (icon_categories, categories_set)
}

fn cargo_template(features: &BTreeMap<String, ()>) -> String {
    let mut template = r#"# GENERATED FILE!
# Edit xtask/src/update.rs to maintain this file

[package]
name = "lumo-icons"
version = "0.8.0"
description = "Lumo icons for leptos"
authors = ["James <lumo.trade>"]
readme = "README.md"
repository = "https://github.com/cornfoo/lumo-icons"
keywords = ["icons", "leptos", "lumo"]
edition = "2021"
license = "MIT"
exclude = ["/core"]

# See more keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

[dependencies]
leptos = { version = "0.7.8", features = ["csr", "nightly"] }
serde_json = "1"

[workspace]
members = ["xtask"]

[features]
default = ["all"]
"#
    .to_string();

    // Add in the "all" feature
    template.push_str(&format!(
        "all = [\n{}\n]\n",
        features
            .iter()
            .map(|(feature, _)| format!("\t\"{feature}\""))
            .collect::<Vec<_>>()
            .join(",\n"),
    ));

    // now add the rest, read from the icon_categories
    for feature in features.keys() {
        template.push_str(&format!("{feature} = []\n"));
    }

    template
}

fn icon_template(
    icon_name: &str,
    icon_styles: impl Iterator<Item = (String, String)>,
) -> TokenStream {
    let component_ident = format_ident!("{}", icon_name.to_case(Case::UpperSnake));
    let styles = icon_styles.map(|s| s.1);

    quote! {
        //! GENERATED FILE
        pub const #component_ident: &crate::IconStyleData = &crate::IconStyleData([#(#styles),*]);
    }
}

const OUTPUT_DIR: &str = "src/icons";
const ASSETS_DIR: &str = "assets";
const TYPESCRIPT_EXPORT_FILE: &str = "metadata/icons.json";

// Critical: normalize any non-native viewBox content to a common canvas so icons render at the correct scale.
fn parse_viewbox(raw: &str) -> Option<(f32, f32)> {
    let re = Regex::new(r#"(?i)viewBox\s*=\s*"[^"]*?0\s+0\s+([0-9.]+)\s+([0-9.]+)""#).unwrap();
    re.captures(raw).map(|c| {
        let w = c[1].parse::<f32>().unwrap_or(256.0);
        let h = c[2].parse::<f32>().unwrap_or(256.0);
        (w, h)
    })
}

fn strip_svg_outer(raw: &str) -> String {
    let svg_open = Regex::new(r"(?is)<svg[^>]*>").unwrap();
    let svg_close = Regex::new(r"(?is)</svg>").unwrap();
    svg_close
        .replace(&svg_open.replace(raw, ""), "")
        .to_string()
}

fn normalize_colors_to_current(inner: &str) -> String {
    let fill_attr = Regex::new(r#"(?i)fill\s*=\s*"([^"]+)""#).unwrap();
    let s = fill_attr.replace_all(inner, |caps: &regex::Captures| {
        let v = &caps[1];
        if v.eq_ignore_ascii_case("none") || v.eq_ignore_ascii_case("currentcolor") {
            caps[0].to_string()
        } else {
            r#"fill="currentColor""#.to_string()
        }
    });
    let stroke_attr = Regex::new(r#"(?i)stroke\s*=\s*"([^"]+)""#).unwrap();
    let s = stroke_attr.replace_all(&s, |caps: &regex::Captures| {
        let v = &caps[1];
        if v.eq_ignore_ascii_case("none") || v.eq_ignore_ascii_case("currentcolor") {
            caps[0].to_string()
        } else {
            r#"stroke="currentColor""#.to_string()
        }
    });
    s.to_string()
}

fn normalize_svg(raw: &str, target_canvas: f32) -> String {
    let (vw, vh) = parse_viewbox(raw).unwrap_or((target_canvas, target_canvas));
    let src_max = vw.max(vh);
    let scale = if (src_max - 0.0).abs() < f32::EPSILON {
        1.0
    } else {
        target_canvas / src_max
    };

    let inner = strip_svg_outer(raw);
    let inner = normalize_colors_to_current(&inner);

    if (scale - 1.0).abs() > f32::EPSILON {
        format!(r#"<g transform="scale({scale})">{inner}</g>"#)
    } else {
        inner
    }
}

pub fn run() {
    // Extract the categories from the typescript export file
    let (icon_categories, categories_set) =
        extract_categories(fs::read_to_string(TYPESCRIPT_EXPORT_FILE).unwrap().as_str());

    let uncategorized = vec!["uncategorized".into()];

    // Clean up the icons folder
    let _ = fs::remove_dir_all(OUTPUT_DIR);
    fs::write("src/lib.rs", "").unwrap();
    fs::create_dir(OUTPUT_DIR).unwrap();

    // Get a list of all the icon styles
    let mut styles: Vec<_> = fs::read_dir(ASSETS_DIR)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();

    // Sort the styles so their ordering is stable.
    styles.sort_unstable();

    // Collect the canonical icon list from all style folders
    let mut file_names_set = std::collections::BTreeSet::new();
    for s in &styles {
        if let Ok(dir) = fs::read_dir(format!("{ASSETS_DIR}/{s}")) {
            for entry in dir.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                        if name.ends_with(".svg") {
                            file_names_set.insert(name.to_string());
                        }
                    }
                }
            }
        }
    }
    let mut file_names: Vec<_> = file_names_set.into_iter().collect();

    // We'll also sort the file names so each generation run has a
    // stable order. This should improve `src/mod.rs` diffs.
    file_names.sort_unstable();

    // Determine a dynamic canvas from the largest source viewBox across all icons.
    let mut canvas: f32 = 256.0;
    for s in &styles {
        if let Ok(dir) = fs::read_dir(format!("{ASSETS_DIR}/{s}")) {
            for entry in dir.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(raw) = fs::read_to_string(&path) {
                        if let Some((w, h)) = parse_viewbox(&raw) {
                            let m = w.max(h);
                            if m > canvas {
                                canvas = m;
                            }
                        }
                    }
                }
            }
        }
    }

    let mut mod_content = Vec::new();
    for file_name in file_names {
        let icon_name = file_name.strip_suffix(".svg").unwrap().to_string();

        //derive the feature set string for this icon from its mappings.
        //If we haven't been able to match the icon's category, assign in to 'Uncategorized'
        let features = icon_categories.get(&icon_name).unwrap_or(&uncategorized);

        let icon_styles = styles.iter().map(|style| {
            let file_name = file_name.clone();
            let path = format!("{ASSETS_DIR}/{style}/{file_name}");
            let svg_raw = fs::read_to_string(&path).unwrap_or_default();
            let svg = normalize_svg(&svg_raw, canvas);
            (style.to_string(), svg)
        });

        let file = icon_template(&icon_name, icon_styles);

        fs::write(
            format!("{OUTPUT_DIR}/{}.rs", icon_name.to_case(Case::Snake)),
            file.to_string(),
        )
        .unwrap();

        let mod_name = format_ident!("{}", icon_name.to_case(Case::Snake));
        if features.len() == 1 {
            let feature = &features[0];
            mod_content.push(quote! {
                #[cfg(feature = #feature)]
                #[doc(hidden)]
                mod #mod_name;

                #[cfg(feature = #feature)]
                #[doc(hidden)]
                pub use #mod_name::*;
            });
        } else {
            mod_content.push(quote! {
                #[cfg(any(#(feature = #features),*))]
                #[doc(hidden)]
                mod #mod_name;

                #[cfg(any(#(feature = #features),*))]
                #[doc(hidden)]
                pub use #mod_name::*;
            });
        }
    }

    let module = quote! { #(#mod_content)* }.to_string();
    fs::write(format!("{OUTPUT_DIR}/mod.rs"), module).unwrap();

    let style_variants: Vec<_> = styles
        .iter()
        .map(|s| format_ident!("{}", s.to_case(Case::UpperCamel)))
        .collect();

    let style_len = style_variants.len();
    let style_indices = style_variants.iter().enumerate().map(|(i, v)| {
        quote! { IconStyle::#v => self.0[#i] }
    });

    let default_variant = style_variants
        .get(0)
        .cloned()
        .unwrap_or_else(|| format_ident!("Regular"));

    let canvas_int = canvas as i32;

    let lib = quote! {
        //! Phosphor is a flexible icon family for interfaces, diagrams,
        //! presentations — whatever, really.
        //! You can explore the available icons at [phosphoricons.com](https://phosphoricons.com).
        //!
        //! ```
        //! use leptos::prelude::*;
        //! use phosphor_leptos::{Icon, IconStyle, HORSE, HEART, CUBE};
        //!
        //! #[component]
        //! fn MyComponent() -> impl IntoView {
        //!     view! {
        //!         <Icon icon=HORSE />
        //!         <Icon icon=HEART color="#AE2983" style=IconStyle::Fill size="32px" />
        //!         <Icon icon=CUBE color="teal" style=IconStyle::Duotone />
        //!     }
        //! }
        //! ```
        use leptos::{prelude::*, text_prop::TextProp};

        mod icons;
        pub use icons::*;

        /// An icon's style.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum IconStyle {
            #(#style_variants),*
        }

        /// The SVG path data for all styles of a particular icon.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct IconStyleData([&'static str; #style_len]);

        impl IconStyleData {
            pub fn get(&self, style: IconStyle) -> &'static str {
                match style {
                    #(#style_indices),*
                }
            }
        }

        pub type IconData = &'static IconStyleData;

        #[component]
        pub fn Icon(
            icon: IconData,
            #[prop(into, default = Signal::stored(IconStyle::#default_variant))] style: Signal<IconStyle>,
            #[prop(into, default = TextProp::from("1em"))] size: TextProp,
            #[prop(into, default = TextProp::from("currentColor"))] color: TextProp,
            #[prop(into, default = Signal::stored(false))] mirrored: Signal<bool>,
            #[prop(optional)] label: Option<&'static str>,
        ) -> impl IntoView {
            let html = move || icon.get(style.get());
            let transform = move || mirrored.get().then_some("scale(-1, 1)");
            let height = size.clone();
            let color_attr = color.clone();

            view! {
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    role="img"
                    aria-label=label.unwrap_or("")
                    width=move || size.get()
                    height=move || height.get()
                    fill=move || color.get()
                    color=move || color_attr.get()
                    transform=transform
                    viewBox=concat!("0 0 ", #canvas_int, " ", #canvas_int)
                >
                    { label.map(|l| view! { <title>{ l }</title> }) }
                    { move || html() }
                </svg>
            }
        }
    };

    fs::write("src/lib.rs", lib.to_string()).expect("Error writing lib file");

    // Write out the newly generated cargo file
    fs::write("Cargo.toml", cargo_template(&categories_set)).unwrap();

    process::Command::new("cargo")
        .arg("fmt")
        .status()
        .expect("Error running cargo fmt");
    process::Command::new("leptosfmt")
        .arg("src")
        .status()
        .expect("Error running leptosfmt");
}
