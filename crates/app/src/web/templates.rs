use crate::web::i18n::Catalog;
use rust_embed::RustEmbed;
use std::collections::HashMap;
use std::sync::Arc;
use tera::{Context, Tera, Value};

#[derive(RustEmbed)]
#[folder = "assets/templates"]
struct TemplateAssets;

#[derive(Clone)]
pub struct Renderer {
    tera: Arc<Tera>,
    catalog: Catalog,
}

impl Renderer {
    pub fn new(catalog: Catalog) -> Self {
        let mut tera = Tera::default();
        let mut raw: Vec<(String, String)> = Vec::new();
        for name in TemplateAssets::iter() {
            let bytes = TemplateAssets::get(name.as_ref()).unwrap().data;
            raw.push((name.to_string(), String::from_utf8(bytes.to_vec()).unwrap()));
        }
        tera.add_raw_templates(raw.iter().map(|(n, s)| (n.as_str(), s.as_str())))
            .expect("templates compile");
        Self {
            tera: Arc::new(tera),
            catalog,
        }
    }

    /// Whether the i18n catalog carries this locale (used to validate a requested
    /// locale before rendering — an unknown one falls back to the default).
    pub fn knows_locale(&self, locale: &str) -> bool {
        self.catalog.has_locale(locale)
    }

    /// Render `template` with a per-request locale + theme; registers a locale-bound `t`.
    pub fn render(
        &self,
        template: &str,
        locale: &str,
        theme_attr: &str,
        mut ctx: Context,
    ) -> Result<String, tera::Error> {
        ctx.insert("locale", locale);
        ctx.insert("theme_attr", theme_attr);
        // Bind a `t(key=...)` Tera function to this locale.
        // NOTE: this deep-clones the whole Tera engine per render so the closure
        // can capture the request locale. The cost is O(all templates registered
        // in the engine), not local to this render — it grows with the entire
        // template set, not the one page. Cheap at foundation scale (a couple of
        // templates). Revisit — pass the locale through the Tera context and read
        // it in the function instead — when htmx fragments multiply the template
        // count, not merely if profiling flags it.
        let catalog = self.catalog.clone();
        let locale_owned = locale.to_string();
        let mut tera = (*self.tera).clone();
        tera.register_function(
            "t",
            move |args: &HashMap<String, Value>| -> tera::Result<Value> {
                let key = args
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| tera::Error::msg("t() needs a `key`"))?;
                Ok(Value::String(catalog.t(&locale_owned, key)))
            },
        );
        tera.render(template, &ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn renderer() -> Renderer {
        Renderer::new(Catalog::load("en"))
    }

    #[test]
    fn renders_shell_in_french() {
        let html = renderer()
            .render("index.html", "fr", "dark", Context::new())
            .unwrap();
        assert!(html.contains("Tableau de bord")); // fr nav label
        assert!(html.contains(r#"lang="fr""#));
        assert!(html.contains(r#"data-theme="dark""#));
    }

    #[test]
    fn renders_shell_in_english_auto_theme() {
        let html = renderer()
            .render("index.html", "en", "", Context::new())
            .unwrap();
        assert!(html.contains("Dashboard"));
        assert!(!html.contains("data-theme")); // Auto => no attribute
    }

    #[test]
    fn defined_keys_do_not_leak_raw() {
        // The shell references only defined keys, so no raw key must appear.
        let html = renderer()
            .render("index.html", "en", "", Context::new())
            .unwrap();
        assert!(!html.contains("nav.dashboard")); // the raw key must NOT leak
    }

    #[test]
    fn undefined_key_surfaces_verbatim_through_render() {
        // Exercise the actual `t` fallback path at the render layer: an UNKNOWN
        // key must render as itself, so template typos fail at `cargo test`,
        // not in prod (brief §6). This proves the fallback the whole i18n
        // design rests on, which the shell templates never trigger.
        let mut r = renderer();
        Arc::get_mut(&mut r.tera)
            .unwrap()
            .add_raw_template("probe.html", r#"{{ t(key="does.not.exist") }}"#)
            .unwrap();
        let html = r.render("probe.html", "en", "", Context::new()).unwrap();
        assert!(html.contains("does.not.exist"));
    }
}
