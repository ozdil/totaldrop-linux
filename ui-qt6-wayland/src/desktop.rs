// SPDX-FileCopyrightText: 2026 The TotalDrop Contributors
// SPDX-License-Identifier: GPL-3.0-or-later
//
//! Desktop services: the portal file chooser, notifications, QR codes,
//! TotalDrop links and code entry helpers.
//!
//! Files are chosen through xdg-desktop-portal, so the desktop's own dialog
//! opens (KDE's under Plasma, GTK's under Hyprland and GNOME) and the app
//! works unchanged inside Flatpak.

#![cfg_attr(
    not(totaldrop_cargo_build),
    allow(unexpected_cfgs, dead_code, clippy::unwrap_used, clippy::expect_used)
)]

#[cfg(totaldrop_cargo_build)]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[cfg(totaldrop_cargo_build)]
use ashpd::desktop::file_chooser::{FileFilter, SelectedFiles};

/// Where received files go: `~/Downloads/TotalDrop` (localized by XDG).
/// The command line's Flatpak default is the same folder: keep it equal to
/// `downloads` in `crates/totaldrop-cli/src/sandbox.rs`.
#[cfg(totaldrop_cargo_build)]
pub fn downloads() -> PathBuf {
    downloads_in(dirs::download_dir(), dirs::home_dir())
}

/// The folder for what `dirs` found, made absolute against the current
/// folder (#441): `dirs` takes a relative `$HOME` as it is, and with no
/// download folder and no home the folder is `./TotalDrop`, which as a
/// `file://` URL names another place. Only if the current folder cannot be
/// read does it stay relative.
pub fn downloads_in(download: Option<PathBuf>, home: Option<PathBuf>) -> PathBuf {
    let dir = download
        .or_else(|| home.map(|h| h.join("Downloads")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TotalDrop");
    std::path::absolute(&dir).unwrap_or(dir)
}

/// A `file://` URL for an absolute path, with every byte outside the
/// unreserved set and `/` percent-encoded, so that `#`, `?` and `%` in a
/// folder's name stay part of the path when Qt reads the URL (#427).
/// A relative path has no `file://` URL (its first part would be read as
/// a host name), so it gives an empty string and the app shows no
/// "Open folder" for it.
pub fn file_url(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt as _;
    if !path.is_absolute() {
        return String::new();
    }
    let mut out = String::from("file://");
    for &b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Decode `%XX` escapes of a file URL path.
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Some(Ok(v)) = s.get(i + 1..i + 3).map(|h| u8::from_str_radix(h, 16))
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Total size of the regular files under `dir`.
pub fn folder_size(dir: &Path) -> u64 {
    let mut total: u64 = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() => {
                    total = total.saturating_add(e.metadata().map(|m| m.len()).unwrap_or(0));
                }
                _ => {}
            }
        }
    }
    total
}

/// The code in what was typed or pasted, in its English form
/// (`7-harbor-rocket`): any letter case, spaces or hyphens, invisible format
/// characters dropped, and found inside a whole share message
/// ([`totaldrop_proto::TransferCode::find`]). `None` if there is no code.
#[cfg(totaldrop_cargo_build)]
pub fn normalize_code(input: &str) -> Option<String> {
    totaldrop_proto::TransferCode::find(input).map(|c| c.to_string())
}

/// Code words that start with `prefix`.
#[cfg(totaldrop_cargo_build)]
pub fn completions(prefix: &str, limit: usize) -> Vec<&'static str> {
    let prefix = prefix.trim().to_lowercase();
    if prefix.is_empty() || prefix.chars().any(|c| !c.is_ascii_lowercase()) {
        return Vec::new();
    }
    (0..2048u16)
        .filter_map(totaldrop_proto::english_word)
        .filter(|w| w.starts_with(&prefix))
        .take(limit)
        .collect()
}

/// The link a QR code carries. The code sits after `#`, so it never
/// reaches a web server.
pub fn link_for(code: &str) -> String {
    format!("https://totaldrop.app/r#{code}")
}

/// The code in a TotalDrop link or a bare code, normalized; strict, so a
/// file name or a text with a date in it does not become a code
/// ([`totaldrop_proto::TransferCode::from_link`], shared with Android).
#[cfg(totaldrop_cargo_build)]
pub fn code_from_link(text: &str) -> Option<String> {
    totaldrop_proto::TransferCode::from_link(text).map(|c| c.to_string())
}

/// QR modules for `text` as a row-major string of `1` (dark) and `0`, and
/// the side length.
#[cfg(totaldrop_cargo_build)]
pub fn qr_modules(text: &str) -> (String, i32) {
    use qrcode::{Color, EcLevel, QrCode};
    let Ok(code) = QrCode::with_error_correction_level(text, EcLevel::M) else {
        return (String::new(), 0);
    };
    let s: String = code
        .to_colors()
        .iter()
        .map(|c| if *c == Color::Dark { '1' } else { '0' })
        .collect();
    (s, i32::try_from(code.width()).unwrap_or(0))
}

/// Read the first QR code in the image at `path`, dark on light or light
/// on dark.
#[cfg(totaldrop_cargo_build)]
pub fn read_qr(path: &Path) -> Option<String> {
    let img = image::open(path).ok()?.to_luma8();
    let try_decode = |img: image::GrayImage| {
        let mut prepared = rqrr::PreparedImage::prepare(img);
        prepared
            .detect_grids()
            .into_iter()
            .find_map(|g| g.decode().ok().map(|(_, content)| content))
    };
    if let Some(c) = try_decode(img.clone()) {
        return Some(c);
    }
    let mut inverted = img;
    image::imageops::invert(&mut inverted);
    try_decode(inverted)
}

#[cfg(totaldrop_cargo_build)]
fn file_path(u: &ashpd::Uri) -> Option<PathBuf> {
    u.as_str()
        .strip_prefix("file://")
        .map(|p| PathBuf::from(percent_decode(p)))
}

/// Ask the desktop's file chooser for files to send.
#[cfg(totaldrop_cargo_build)]
pub async fn pick_files(title: &str) -> Vec<String> {
    let request = SelectedFiles::open_file()
        .title(title)
        .modal(true)
        .multiple(true)
        .send()
        .await;
    let Ok(request) = request else {
        return Vec::new();
    };
    let Ok(files) = request.response() else {
        return Vec::new();
    };
    files
        .uris()
        .iter()
        .filter_map(file_path)
        .map(|p| p.display().to_string())
        .collect()
}

/// Ask the desktop's file chooser for one image.
#[cfg(totaldrop_cargo_build)]
pub async fn pick_image(title: &str) -> Option<PathBuf> {
    let images = FileFilter::new("Images")
        .mimetype("image/png")
        .mimetype("image/jpeg")
        .mimetype("image/webp");
    let request = SelectedFiles::open_file()
        .title(title)
        .modal(true)
        .multiple(false)
        .filter(images)
        .send()
        .await
        .ok()?;
    let files = request.response().ok()?;
    files.uris().first().and_then(file_path)
}

/// A desktop notification through `org.freedesktop.Notifications`, which
/// every desktop answers (Plasma, mako and swaync on Hyprland, GNOME).
#[cfg(totaldrop_cargo_build)]
pub async fn notify(title: String, body: String) {
    let Ok(conn) = zbus::Connection::session().await else {
        return;
    };
    let hints: HashMap<&str, zbus::Variant<'_>> =
        HashMap::from([("desktop-entry", zbus::Variant::from(crate::APP_ID))]);
    let _ = conn
        .call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &(
                "TotalDrop",
                0u32,
                crate::APP_ID,
                title.as_str(),
                body.as_str(),
                Vec::<&str>::new(),
                hints,
                -1i32,
            ),
        )
        .await;
}

/// Pure-Rust desktop session classification (`XDG_CURRENT_DESKTOP`,
/// `QT_QPA_PLATFORMTHEME`, `TOTALDROP_THEME`, and Omarchy state presence),
/// shared with host unit testing when Qt 6 is not installed.
pub fn resolve_theme_source(
    forced: Option<&str>,
    xdg_desktop: &str,
    platform_theme: &str,
    omarchy_present: bool,
) -> &'static str {
    match forced {
        Some("omarchy") => return "omarchy",
        Some("system" | "kde") => return "system",
        Some("portal" | "hyprland") => return "portal",
        Some("aurora") => return "aurora",
        _ => {}
    }
    let is_kde = xdg_desktop
        .split(':')
        .any(|d| d.eq_ignore_ascii_case("kde"));
    if is_kde {
        return "system";
    }
    if omarchy_present {
        return "omarchy";
    }
    let qpa = platform_theme.to_ascii_lowercase();
    let has_qt_palette = ["kde", "qt6ct", "qt5ct", "hyprqt6engine"]
        .iter()
        .any(|known| qpa.split(';').any(|p| p.trim() == *known));
    if has_qt_palette { "system" } else { "portal" }
}

#[cfg(all(test, not(totaldrop_cargo_build)))]
#[path = "theme/color.rs"]
mod standalone_color;

#[cfg(test)]
mod tests {
    use super::*;

    /// #427: "Open folder" opens the folder itself, also when its name has
    /// characters a URL gives a meaning to. Qt reads the URL; before, the
    /// QML built `"file://" + folder`, which Qt cut at `#` and `?` and
    /// decoded at `%`.
    #[cfg(totaldrop_cargo_build)]
    #[test]
    fn a_folder_url_names_the_folder_for_qt() {
        use cxx_qt_lib::{QString, QUrl};
        let local = |url: &str| {
            QUrl::from(&QString::from(url))
                .to_local_file()
                .map(|p| p.to_string())
        };
        for folder in [
            "/home/o/Downloads/Project #2",
            "/home/o/a?b",
            "/home/o/100%25 done",
            "/home/o/50% off",
            "/home/o/two  spaces",
            "/home/o/a\nb",
            "/home/o/🙂",
            "/home/o/plain",
        ] {
            assert_eq!(
                local(&file_url(Path::new(folder))).as_deref(),
                Some(folder),
                "{folder}"
            );
        }
    }

    /// A relative path has no URL: `file://TotalDrop` would name a host.
    #[test]
    fn a_relative_folder_has_no_url() {
        assert_eq!(file_url(Path::new("TotalDrop")), "");
        assert_eq!(file_url(Path::new("./TotalDrop")), "");
        assert_eq!(file_url(Path::new("/TotalDrop")), "file:///TotalDrop");
    }

    /// The URL is plain ASCII and round-trips through the app's own reader
    /// of `file://` URLs, for every ASCII character a file name may hold
    /// and for letters outside ASCII.
    #[test]
    fn a_folder_url_round_trips() {
        let mut name: String = (1u8..=127).filter(|&b| b != b'/').map(char::from).collect();
        name.push_str("İçğüşö 文件 🙂");
        let path = format!("/tmp/{name}");
        let url = file_url(Path::new(&path));
        assert!(url.is_ascii());
        assert!(!url.contains(['#', '?', ' ']));
        assert_eq!(percent_decode(url.strip_prefix("file://").unwrap()), path);
    }

    #[test]
    fn the_downloads_folder_is_always_absolute() {
        // #441: a relative folder became `file://./TotalDrop`, and "Open
        // folder" asked for host `.`.
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(downloads_in(None, None), cwd.join("TotalDrop"));
        assert_eq!(
            downloads_in(None, Some(PathBuf::from("ada"))),
            cwd.join("ada/Downloads/TotalDrop")
        );
        assert_eq!(
            downloads_in(Some(PathBuf::from("dl")), None),
            cwd.join("dl/TotalDrop")
        );
        assert_eq!(
            downloads_in(
                Some(PathBuf::from("/home/ada/Downloads-Custom")),
                Some(PathBuf::from("/home/ada"))
            ),
            PathBuf::from("/home/ada/Downloads-Custom/TotalDrop")
        );
        assert_eq!(
            downloads_in(None, Some(PathBuf::from("/home/ada"))),
            PathBuf::from("/home/ada/Downloads/TotalDrop")
        );
    }

    #[test]
    fn folder_size_sums_nested_regular_files() {
        let dir = std::env::temp_dir().join(format!("td-desktop-size-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        std::fs::write(dir.join("a.bin"), [1u8; 17]).unwrap();
        std::fs::write(dir.join("nested/b.bin"), [2u8; 25]).unwrap();
        assert_eq!(folder_size(&dir), 42);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn share_link_uses_url_fragment() {
        assert_eq!(
            link_for("7-harbor-rocket"),
            "https://totaldrop.app/r#7-harbor-rocket"
        );
    }

    #[test]
    fn detects_wayland_desktop_theme_sources() {
        assert_eq!(resolve_theme_source(None, "KDE", "", true), "system");
        assert_eq!(resolve_theme_source(None, "Hyprland", "", true), "omarchy");
        assert_eq!(
            resolve_theme_source(None, "Hyprland", "hyprqt6engine", false),
            "system"
        );
        assert_eq!(
            resolve_theme_source(None, "sway:wlroots", "qt6ct", false),
            "system"
        );
        assert_eq!(resolve_theme_source(None, "Hyprland", "", false), "portal");
        assert_eq!(
            resolve_theme_source(None, "ubuntu:GNOME", "", false),
            "portal"
        );
        assert_eq!(
            resolve_theme_source(Some("aurora"), "KDE", "kde", true),
            "aurora"
        );
    }

    #[cfg(totaldrop_cargo_build)]
    #[test]
    fn codes_from_links_and_typing() {
        assert_eq!(
            normalize_code(" 7 Abandon  ability "),
            Some("7-abandon-ability".into())
        );
        assert_eq!(normalize_code("seven words"), None);
        assert_eq!(
            code_from_link("https://totaldrop.app/r#7-abandon-ability"),
            Some("7-abandon-ability".into())
        );
        assert_eq!(
            code_from_link("totaldrop://r/12-zoo-zone-able"),
            Some("12-zoo-zone-able".into())
        );
        assert_eq!(code_from_link("https://example.com"), None);
        for text in [
            "/home/u/Photos/2024-10-30/beach.jpg",
            "IMG 7-abandon-ability.jpg",
            "Get my files with TotalDrop. Code: 7-abandon-ability",
        ] {
            assert_eq!(code_from_link(text), None, "{text:?}");
        }
    }

    #[cfg(totaldrop_cargo_build)]
    #[test]
    fn codes_typed_with_arabic_indic_digits() {
        for code in ["\u{0667}-abandon-ability", "\u{06F7} abandon ability"] {
            assert_eq!(
                normalize_code(code),
                Some("7-abandon-ability".into()),
                "{code:?}"
            );
        }
    }

    #[cfg(totaldrop_cargo_build)]
    #[test]
    fn codes_in_a_pasted_share_message() {
        for text in [
            "Get my files with TotalDrop. Code: \u{2068}7-abandon-ability\u{2069}",
            "استلم ملفاتي عبر TotalDrop. الرمز: \u{2068}7-abandon-ability\u{2069}",
        ] {
            assert_eq!(
                normalize_code(text),
                Some("7-abandon-ability".into()),
                "{text:?}"
            );
        }
    }

    #[cfg(totaldrop_cargo_build)]
    #[test]
    fn codes_pasted_from_a_right_to_left_message() {
        for code in [
            "\u{2068}7-abandon-ability\u{2069}",
            "\u{2066}7 abandon ability\u{2069}",
            "\u{2067}7-abandon-ability\u{2069}\u{200F}",
            "\u{200E}7-\u{202D}abandon\u{202C}-ability\u{061C}",
        ] {
            assert_eq!(
                normalize_code(code),
                Some("7-abandon-ability".into()),
                "{code:?}"
            );
        }
        assert_eq!(normalize_code("\u{2068}\u{2069}"), None);
        for link in [
            "\u{2068}totaldrop://r/7-abandon-ability\u{2069}",
            "\u{2068}https://totaldrop.app/r#7-abandon-ability\u{2069}",
            "totaldrop://r/\u{2068}7-abandon-ability\u{2069}",
            "totaldrop://r/%E2%81%A87-abandon-ability%E2%81%A9",
            "\u{FEFF}totaldrop://r/7-aban\u{200B}don-ability",
        ] {
            assert_eq!(
                code_from_link(link),
                Some("7-abandon-ability".into()),
                "{link:?}"
            );
        }
    }

    #[cfg(totaldrop_cargo_build)]
    #[test]
    fn completes_words() {
        assert_eq!(completions("abo", 5), vec!["about", "above"]);
        assert!(completions("", 5).is_empty());
        assert!(completions("Ä", 5).is_empty());
    }

    #[cfg(totaldrop_cargo_build)]
    #[test]
    fn qr_round_trip() {
        let (m, n) = qr_modules(&link_for("7-abandon-ability"));
        assert!(n >= 21);
        assert_eq!(m.len(), (n * n) as usize);
    }

    #[test]
    fn decodes_file_urls() {
        assert_eq!(percent_decode("/home/a%20b/%C3%A7.txt"), "/home/a b/ç.txt");
        assert_eq!(percent_decode("100%"), "100%");
    }
}
