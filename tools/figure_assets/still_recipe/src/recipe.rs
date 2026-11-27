//! Compile the body, material, ordered fit and style recipe used by every native consumer.
use std::error::Error;

use gibson::figure::perform::Style;
use gibson::figure::studio::{AdaptationV1, GarmentV1, MaterialSlotV1, RecipeV1};
use serde_json::{from_value, Value};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub(crate) fn movement_style(preset: &Value) -> Result<Style> {
    let selected = preset["render"]
        .get("movement_style")
        .and_then(Value::as_str)
        .unwrap_or("restrained");
    match selected {
        "restrained" => Ok(Style::restrained()),
        "expressive" => Ok(Style::expressive()),
        _ => Err(format!("unsupported movement style {selected}").into()),
    }
}

pub(crate) fn build_recipe(preset: &Value, style: &Style) -> Result<RecipeV1> {
    if preset["name"].as_str().filter(|s| !s.is_empty()).is_none() || preset["rig"] != "game_engine"
    {
        return Err("a named preset and the game_engine rig are required".into());
    }
    let mut recipe = RecipeV1::new();
    let controls = preset["controls"].as_object().ok_or("missing controls")?;
    for (name, value) in controls {
        recipe.set_control(name, from_value(value.clone())?);
    }
    let render = &preset["render"];
    let materials = render["materials"].as_object().ok_or("missing materials")?;
    for (slot, spec) in materials {
        let mut m = MaterialSlotV1::new(slot);
        m.albedo = from_value(spec["albedo"].clone())?;
        m.roughness = from_value(spec["roughness"].clone())?;
        m.metalness = from_value(spec["metalness"].clone())?;
        if let Some(color) = spec.get("emissive") {
            m.emissive = Some((
                from_value(color.clone())?,
                from_value(spec["emissive_intensity"].clone())?,
            ));
        }
        recipe.materials.push(m);
    }
    recipe.hair.style_id = Some(from_value(render["hair"]["id"].clone())?);
    recipe.hair.color = from_value(render["hair"]["color"].clone())?;
    let garment_ids: Vec<String> = from_value(render["garments"].clone())?;
    let bindings = render["bindings"]
        .as_object()
        .ok_or("missing garment bindings")?;
    for id in garment_ids {
        let kind = if id == "skirt_a_line" {
            "skirt"
        } else {
            "garment"
        };
        let slot = bindings
            .get(&format!("garment:{id}"))
            .or_else(|| bindings.get(kind))
            .and_then(Value::as_str)
            .ok_or_else(|| format!("missing slot binding for {id}"))?;
        if !materials.contains_key(slot) {
            return Err(format!("undefined slot {slot} on {id}").into());
        }
        let mut garment = GarmentV1::new(id);
        garment.slot_bindings.push((String::new(), slot.to_owned()));
        recipe.garments.push(garment);
    }
    recipe.fit_params.adaptation = Some(AdaptationV1::default());
    recipe.motion_style.preset = style.preset.clone();
    recipe.motion_style.amplitude = style.amplitude;
    recipe.motion_style.tempo = style.tempo;
    recipe.motion_style.settle_ease_ms = style.settle_ms as f32;
    Ok(recipe)
}
