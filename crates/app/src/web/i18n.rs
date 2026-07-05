use rust_embed::RustEmbed;
use std::collections::HashMap;

#[derive(RustEmbed)]
#[folder = "assets/i18n"]
struct I18nAssets;

/// All locale catalogs loaded from embedded JSON at startup.
#[derive(Clone)]
pub struct Catalog {
    locales: HashMap<String, HashMap<String, String>>,
    default_locale: String,
}

impl Catalog {
    pub fn load(default_locale: &str) -> Self {
        let mut locales = HashMap::new();
        for file in I18nAssets::iter() {
            let name = file.as_ref();
            if let Some(code) = name.strip_suffix(".json") {
                let bytes = I18nAssets::get(name).unwrap().data;
                let map: HashMap<String, String> =
                    serde_json::from_slice(&bytes).expect("valid i18n json");
                locales.insert(code.to_string(), map);
            }
        }
        Self {
            locales,
            default_locale: default_locale.to_string(),
        }
    }

    /// Look up a key in `locale`, falling back to the default locale, then the key itself.
    pub fn t(&self, locale: &str, key: &str) -> String {
        self.locales
            .get(locale)
            .and_then(|m| m.get(key))
            .or_else(|| {
                self.locales
                    .get(&self.default_locale)
                    .and_then(|m| m.get(key))
            })
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }

    pub fn has_locale(&self, locale: &str) -> bool {
        self.locales.contains_key(locale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_known_key_per_locale() {
        let c = Catalog::load("en");
        assert_eq!(c.t("fr", "nav.spools"), "Bobines");
        assert_eq!(c.t("en", "nav.spools"), "Spools");
    }

    #[test]
    fn falls_back_to_default_then_key() {
        let c = Catalog::load("en");
        assert_eq!(c.t("de", "nav.spools"), "Spools"); // unknown locale -> default
        assert_eq!(c.t("en", "missing.key"), "missing.key"); // unknown key -> key
    }
}
