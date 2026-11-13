//! The product's identity in the UI: its name, and the mark at the left of the title bar.
//!
//! This fork ships as SilicaLayr: a fork of LayerShop, itself a fork of PhotoCraft. The code
//! and the translation catalogs it inherits say "PhotoCraft", and the strings this fork's own
//! code added say "LayerShop"; [`display`] is the one place that turns either into [`NAME`] on
//! the way to the screen, so neither the catalogs nor upstream's strings need editing.
//!
//! The mark is the app icon (`assets/app-icon/`, see its README), where Photoshop shows its
//! "Ps" tile. The PNG carries the icon's rounded corners; it is decoded once per context into a
//! mipmapped texture, so it stays crisp at the title bar's size on any display scale.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};

use egui::{Color32, Context, Id, Rect, TextureHandle, TextureOptions, Ui, pos2};
use photocraft_codecs::{ChannelLayout, SampleType};

/// The product's name as users see it. The brand is spelled without the second "e"; machine
/// names (crates, binaries, bundle ids) keep theirs and are not this constant's business.
pub const NAME: &str = "SilicaLayr";

/// The name the UI strings and their translations were written with.
const SOURCE_NAME: &str = "PhotoCraft";

/// The name the project this one forks used, kept in the strings this fork's own code added.
const PREDECESSOR_NAME: &str = "LayerShop";

/// The credit the About window gives the projects this one is built from. Shown as written: it
/// does not pass through [`display`], which would put this build's own name in it.
pub const BASED_ON: &str =
    "Built on the PhotoCraft engine by the ArtCraft team, with workflows from LayerShop by Slavomir Durej. The Apple Silicon port is SilicaLayr's own.";

/// At most this many distinct strings are rewritten and kept. UI strings that name the product
/// number in the low hundreds; the cap is for text that turns out not to be a fixed UI string.
const MAX_RENAMED: usize = 4096;

/// `s` as this build shows it: with [`NAME`] where the string names the product, under either
/// name it inherits ("PhotoCraft" from upstream, "LayerShop" from the fork this one forks).
/// Strings that don't name it come back as they are. Each distinct string that does is
/// rewritten once and kept for the life of the process, which is what lets this hand back a
/// plain `&str` on the path every label takes every frame.
pub fn display(s: &str) -> &str {
    // A string naming neither predecessor, and a build whose name already is both of them,
    // come back as they are.
    if (!s.contains(SOURCE_NAME) && !s.contains(PREDECESSOR_NAME)) || (NAME == SOURCE_NAME && NAME == PREDECESSOR_NAME) {
        return s;
    }
    static RENAMED: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let mut renamed = RENAMED.get_or_init(Default::default).lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(done) = renamed.get(s) {
        return done;
    }
    if renamed.len() >= MAX_RENAMED {
        return s;
    }
    let done: &'static str = Box::leak(s.replace(SOURCE_NAME, NAME).replace(PREDECESSOR_NAME, NAME).into_boxed_str());
    renamed.insert(s.to_string(), done);
    done
}

/// 128 px: sharp at 20 pt on a 3× display, and the mipmaps keep it clean at 1×.
const ICON_PNG: &[u8] = include_bytes!("../../../../assets/app-icon/hicolor/128x128/apps/ai.storyteller.photocraft.png");

/// The icon's pixels (one transparent pixel if it couldn't be decoded, which a test rules out).
fn decode() -> egui::ColorImage {
    let rgba = photocraft_codecs::decode(ICON_PNG).ok().map(|img| img.convert(ChannelLayout::Rgba, SampleType::U8));
    match rgba {
        Some(img) if img.data().len() == img.width() as usize * img.height() as usize * 4 => {
            egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.data())
        }
        _ => egui::ColorImage::filled([1, 1], Color32::TRANSPARENT),
    }
}

/// The icon texture, uploaded on first use and kept in the context.
fn texture(ctx: &Context) -> TextureHandle {
    let id = Id::unique("photocraft-brand-mark");
    if let Some(tex) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return tex;
    }
    let options = TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..TextureOptions::LINEAR };
    let tex = ctx.load_texture("photocraft-brand-mark", decode(), options);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    tex
}

/// Where the mark was painted this frame (tests and screenshots).
fn rect_id() -> Id {
    Id::unique("photocraft-brand-mark-rect")
}

/// Paint the mark into `r` (square).
pub fn paint_mark(ui: &Ui, r: Rect) {
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    ui.painter().image(texture(ui.ctx()).id(), r, uv, Color32::WHITE);
    ui.ctx().data_mut(|d| d.insert_temp(rect_id(), r));
}

/// The rect the mark was last painted into.
#[cfg(test)]
pub fn mark_rect(ctx: &Context) -> Option<Rect> {
    ctx.data(|d| d.get_temp(rect_id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_puts_the_products_name_in_ui_strings() {
        assert_eq!(display("About PhotoCraft"), format!("About {NAME}"));
        assert_eq!(display("PhotoCraft"), NAME);
        // A translation keeps its own words around the name.
        assert_eq!(display("Über PhotoCraft"), format!("Über {NAME}"));
        // Strings that don't name the product are handed back untouched, not copied.
        let plain = "Layer Style";
        assert!(std::ptr::eq(display(plain), plain));
        // The same string is rewritten once.
        assert!(std::ptr::eq(display("Quit PhotoCraft"), display("Quit PhotoCraft")));
    }

    #[test]
    fn display_renames_the_inherited_and_the_fork_names() {
        // Upstream's name, and the name the fork this one forks left in its own strings.
        assert_eq!(display("Restart PhotoCraft"), format!("Restart {NAME}"));
        assert_eq!(display("LayerShop setup"), format!("{NAME} setup"));
        // A string carrying both names comes out as the product's own name throughout.
        assert_eq!(display("PhotoCraft workflows, kept by LayerShop"), format!("{NAME} workflows, kept by {NAME}"));
        // Untouched strings are still handed back as they are, not copied.
        let plain = "Local AI setup";
        assert!(std::ptr::eq(display(plain), plain));
    }

    #[test]
    fn the_about_credit_names_every_project_in_lineage() {
        // BASED_ON is shown as written (About ▸ General hands it to the screen directly), so
        // `display` must not have folded its sources' names into this build's own.
        assert!(BASED_ON.contains(SOURCE_NAME) && BASED_ON.contains(PREDECESSOR_NAME), "{BASED_ON}");
        assert!(BASED_ON.contains(NAME), "{BASED_ON}");
    }

    #[test]
    fn the_icon_decodes_to_a_rounded_128_px_tile() {
        let img = decode();
        assert_eq!(img.size, [128, 128]);
        // Rounded corners: the corner pixel is transparent, the centre opaque.
        assert_eq!(img.pixels[0].a(), 0);
        assert_eq!(img.pixels[64 * 128 + 64].a(), 255);
    }

    #[test]
    fn the_title_bar_paints_the_icon_from_one_cached_texture() {
        let ctx = Context::default();
        crate::PhotocraftApp::setup_context(&ctx, crate::theme::ThemeKind::ALL[0]);
        let mut app = crate::PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        // The system title bar hides the in-app mark (the OS draws its own icon), so paint the
        // app-drawn bar here.
        app.custom_titlebar = true;
        let mut frame = || {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| crate::panels::title_bar(&mut app, ui));
            let uploads = out.textures_delta.set.values().flat_map(|d| d.iter()).filter(|d| d.image.size() == [128, 128]).count();
            out.textures_delta.clear();
            uploads
        };
        assert_eq!(frame(), 1, "uploaded on first use");
        assert_eq!(frame(), 0, "then reused");
        assert!(mark_rect(&ctx).is_some());
    }
}
