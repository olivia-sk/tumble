//! The default registry offers exactly what PRD section 7 promises for the
//! engines present on this machine, and never more.

use std::collections::HashSet;
use tumble_core::format::FORMATS;
use tumble_core::menu::{MENU_MAX, menu_targets};
use tumble_core::{Format, FormatId, Registry};
use tumble_engines::default_registry;

/// Every format at least one available engine can write.
fn writable(registry: &Registry) -> HashSet<FormatId> {
    registry.engines().flat_map(|e| e.steps()).map(|s| s.to).collect()
}

#[test]
fn targets_match_section_7() {
    let registry = default_registry();
    let writable = writable(&registry);
    for f in FORMATS {
        let mut got = registry.targets(f.id);
        let declared = f.declared_targets();
        for t in &got {
            assert!(declared.contains(t), "{} offers undeclared target {}", f.id, t);
        }
        if registry.reads(f.id) {
            let mut want: Vec<FormatId> =
                declared.into_iter().filter(|t| writable.contains(t)).collect();
            want.sort();
            got.sort();
            assert_eq!(got, want, "targets for {}", f.id);
        } else {
            assert!(got.is_empty(), "{} is not read by any engine", f.id);
        }
    }
}

#[test]
fn built_in_image_inputs_are_always_read() {
    let registry = default_registry();
    for id in tumble_engines::raster::READS {
        assert!(registry.reads(FormatId(id)), "{id} is not readable");
    }
}

#[test]
fn vendor_engines_cover_heic_and_pdf_when_present() {
    let registry = default_registry();
    let names: Vec<&str> = registry.engines().map(|e| e.name()).collect();
    if names.contains(&"libheif") {
        assert!(registry.reads(FormatId("heic")));
    }
    if names.contains(&"pdfium") {
        assert!(registry.reads(FormatId("pdf")));
    }
}

#[test]
fn image_routes_are_direct() {
    let registry = default_registry();
    for (from, to) in [("svg", "jpeg"), ("png", "webp")] {
        let route = registry.route(FormatId(from), FormatId(to)).unwrap();
        assert_eq!(route.hops.len(), 1, "{from} -> {to}");
    }
    if registry.reads(FormatId("pdf")) && registry.reads(FormatId("heic")) {
        let route = registry.route(FormatId("pdf"), FormatId("heic")).unwrap();
        assert_eq!(route.hops.len(), 1, "pdf -> heic must not need per-page intermediates");
    }
}

#[test]
fn menus_are_short_and_reachable() {
    let registry = default_registry();
    let photo = Format::by_id("jpeg").unwrap();
    let menu: Vec<_> = menu_targets(&registry, photo).iter().map(|f| f.0).collect();
    let mut want = vec!["png", "webp", "avif", "heic", "gif", "tiff", "ico"];
    if !writable(&registry).contains(&FormatId("heic")) {
        want.retain(|f| *f != "heic");
    }
    assert_eq!(menu, want);
    for f in FORMATS {
        assert!(menu_targets(&registry, f).len() <= MENU_MAX);
    }
}
