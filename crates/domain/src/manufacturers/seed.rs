use crate::manufacturers::model::{ManufacturerName, NewManufacturer};

fn manufacturer(name: &str, country: &str) -> NewManufacturer {
    NewManufacturer {
        name: ManufacturerName::new(name).expect("builtin manufacturer name is non-blank"),
        country: if country.is_empty() {
            None
        } else {
            Some(country.to_string())
        },
    }
}

/// The built-in manufacturer referential — a curated subset of the
/// OpenPrintTag brand database (common filament brands). Starting points;
/// the operator adds their own via the UI. Country is ISO-3166 alpha-2.
pub fn builtin() -> Vec<NewManufacturer> {
    vec![
        manufacturer("Prusament", "CZ"),
        manufacturer("Polymaker", "CN"),
        manufacturer("Bambu Lab", "CN"),
        manufacturer("eSun", "CN"),
        manufacturer("Sunlu", "CN"),
        manufacturer("Creality", "CN"),
        manufacturer("Elegoo", "CN"),
        manufacturer("Overture", "CN"),
        manufacturer("Hatchbox", "US"),
        manufacturer("MatterHackers", "US"),
        manufacturer("Proto-pasta", "US"),
        manufacturer("ColorFabb", "NL"),
        manufacturer("Fillamentum", "CZ"),
        manufacturer("Formfutura", "NL"),
        manufacturer("Extrudr", "AT"),
        manufacturer("Fiberlogy", "PL"),
        manufacturer("Spectrum", "PL"),
        manufacturer("Devil Design", "PL"),
        manufacturer("3DJake", "AT"),
        manufacturer("Das Filament", "DE"),
        manufacturer("Recreus", "ES"),
        manufacturer("Add:North", "SE"),
        manufacturer("BASF", "DE"),
        manufacturer("Nanovia", "FR"),
        manufacturer("Francofil", "FR"),
        manufacturer("Kimya", "FR"),
        manufacturer("Amolen", "CN"),
        manufacturer("Geeetech", "CN"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_names_are_all_non_blank_and_unique() {
        let all = builtin();
        let mut names: Vec<&str> = all.iter().map(|m| m.name.as_str()).collect();
        assert!(names.iter().all(|n| !n.is_empty()));
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(
            before,
            names.len(),
            "builtin manufacturer names must be unique"
        );
    }
}
