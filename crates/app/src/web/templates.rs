use crate::web::i18n::Catalog;
use rust_embed::RustEmbed;
use std::sync::Arc;
use tera::{Context, Kwargs, State, Tera, TeraResult};

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

        // Tera 2 validates at add-time that every function a template calls is
        // already registered, so `t` must be wired up *before* the embedded
        // templates are loaded (v1 -> v2: registration order is no longer
        // "whenever", it's "before add_raw_template(s)"). The closure reads the
        // active locale back out of the per-render `State` (bound via
        // `ctx.insert("locale", ...)` in `render`), so — unlike the old
        // per-render `Tera` clone this replaces — the engine is built once.
        let t_catalog = catalog.clone();
        tera.register_function(
            "t",
            move |kwargs: Kwargs, state: &State| -> TeraResult<String> {
                let key = kwargs.must_get::<&str>("key")?;
                let locale = state.get::<String>("locale")?.unwrap_or_default();
                Ok(t_catalog.t(&locale, key))
            },
        );

        let raw: Vec<(String, String)> = TemplateAssets::iter()
            .map(|name| {
                let bytes = TemplateAssets::get(name.as_ref()).unwrap().data;
                (name.to_string(), String::from_utf8(bytes.to_vec()).unwrap())
            })
            .collect();
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

    /// Translate a single key for `locale` outside of a full template render
    /// — used for the plain-text localized 404 body on an unknown spool id,
    /// where there is no page shell to render into.
    pub fn t(&self, locale: &str, key: &str) -> String {
        self.catalog.t(locale, key)
    }

    /// Render `template` with a per-request locale + theme; `t(key=...)` reads
    /// the locale back out of this context (function registered once, at
    /// construction — see `new`).
    pub fn render(
        &self,
        template: &str,
        locale: &str,
        theme_attr: &str,
        mut ctx: Context,
    ) -> Result<String, tera::Error> {
        ctx.insert("locale", locale);
        ctx.insert("theme_attr", theme_attr);
        self.tera.render(template, &ctx)
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
