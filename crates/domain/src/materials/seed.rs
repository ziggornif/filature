use crate::materials::model::{
    Density, DryingParams, MaterialName, NewMaterial, Sensitivity, Temperature,
};

fn material(
    name: &str,
    density: f64,
    dry_t: u16,
    dry_h: u16,
    sensitivity: Sensitivity,
    nozzle: u16,
    bed: u16,
) -> NewMaterial {
    NewMaterial {
        name: MaterialName::new(name).expect("builtin material name is non-blank"),
        density: Density::new(density).expect("builtin density > 0"),
        drying: DryingParams {
            temp: Temperature::new(dry_t),
            time_h: dry_h,
        },
        sensitivity,
        nozzle: Temperature::new(nozzle),
        bed: Temperature::new(bed),
    }
}

/// The built-in material referential. Starting points; the user tunes per brand.
pub fn builtin() -> Vec<NewMaterial> {
    use Sensitivity::*;
    vec![
        material("PLA", 1.24, 45, 6, Low, 210, 60),
        material("PLA-CF", 1.30, 45, 6, Low, 220, 60),
        material("PETG", 1.27, 65, 6, Medium, 240, 80),
        material("PETG-CF", 1.30, 65, 6, Medium, 250, 80),
        material("ASA", 1.07, 70, 4, Medium, 250, 100),
        material("ABS", 1.04, 70, 4, Medium, 245, 100),
        material("HIPS", 1.04, 65, 4, Medium, 240, 100),
        material("PP", 0.90, 60, 4, Low, 230, 85),
        material("TPU", 1.21, 55, 6, High, 225, 40),
        material("PVA", 1.23, 45, 6, High, 200, 60),
        material("PA", 1.14, 80, 8, High, 260, 90),
        material("PA-CF", 1.16, 80, 8, High, 270, 90),
        material("PA-GF", 1.20, 80, 8, High, 270, 90),
        material("PC", 1.20, 90, 6, High, 270, 110),
    ]
}
