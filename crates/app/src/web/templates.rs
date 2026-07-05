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
    fn missing_i18n_key_would_surface() {
        // A template referencing an undefined key renders the key verbatim,
        // so render tests catch typos at `cargo test`, not in prod (brief §6).
        let html = renderer()
            .render("index.html", "en", "", Context::new())
            .unwrap();
        assert!(!html.contains("nav.dashboard")); // the raw key must NOT leak
    }
}
