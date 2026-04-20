//! Curated window-placement presets. Frame coords are fractions of screen [0..1].

#[derive(Debug, Clone, Copy)]
pub struct Preset {
    pub name: &'static str,
    pub display_name: &'static str,
    pub keywords: &'static [&'static str],
    pub frame: PresetFrame,
}

#[derive(Debug, Clone, Copy)]
pub struct PresetFrame {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub const PRESETS: &[Preset] = &[
    // Halves
    Preset { name: "left-half",   display_name: "Left Half",   keywords: &["left","half"],     frame: PresetFrame{x:0.0,y:0.0,w:0.5,h:1.0} },
    Preset { name: "right-half",  display_name: "Right Half",  keywords: &["right","half"],    frame: PresetFrame{x:0.5,y:0.0,w:0.5,h:1.0} },
    Preset { name: "top-half",    display_name: "Top Half",    keywords: &["top","half","up"], frame: PresetFrame{x:0.0,y:0.0,w:1.0,h:0.5} },
    Preset { name: "bottom-half", display_name: "Bottom Half", keywords: &["bottom","half","down"], frame: PresetFrame{x:0.0,y:0.5,w:1.0,h:0.5} },
    // Thirds (vertical bands)
    Preset { name: "left-third",   display_name: "Left Third",   keywords: &["left","third","1/3"],  frame: PresetFrame{x:0.0,y:0.0,w:0.333,h:1.0} },
    Preset { name: "center-third", display_name: "Center Third", keywords: &["center","third"],      frame: PresetFrame{x:0.333,y:0.0,w:0.334,h:1.0} },
    Preset { name: "right-third",  display_name: "Right Third",  keywords: &["right","third"],       frame: PresetFrame{x:0.667,y:0.0,w:0.333,h:1.0} },
    Preset { name: "left-two-thirds",  display_name: "Left Two-Thirds",  keywords: &["left","two","thirds","2/3"],  frame: PresetFrame{x:0.0,y:0.0,w:0.667,h:1.0} },
    Preset { name: "right-two-thirds", display_name: "Right Two-Thirds", keywords: &["right","two","thirds","2/3"], frame: PresetFrame{x:0.333,y:0.0,w:0.667,h:1.0} },
    Preset { name: "middle-column",    display_name: "Middle Column (2/3)", keywords: &["middle","column"],       frame: PresetFrame{x:0.166,y:0.0,w:0.667,h:1.0} },
    // Quarters
    Preset { name: "top-left-quarter",     display_name: "Top Left Quarter",     keywords: &["top","left","quarter"],     frame: PresetFrame{x:0.0,y:0.0,w:0.5,h:0.5} },
    Preset { name: "top-right-quarter",    display_name: "Top Right Quarter",    keywords: &["top","right","quarter"],    frame: PresetFrame{x:0.5,y:0.0,w:0.5,h:0.5} },
    Preset { name: "bottom-left-quarter",  display_name: "Bottom Left Quarter",  keywords: &["bottom","left","quarter"],  frame: PresetFrame{x:0.0,y:0.5,w:0.5,h:0.5} },
    Preset { name: "bottom-right-quarter", display_name: "Bottom Right Quarter", keywords: &["bottom","right","quarter"], frame: PresetFrame{x:0.5,y:0.5,w:0.5,h:0.5} },
    // Maximize / center / restore variants
    Preset { name: "maximize",         display_name: "Maximize",         keywords: &["max","maximize","full"],            frame: PresetFrame{x:0.0,y:0.0,w:1.0,h:1.0} },
    Preset { name: "almost-maximize",  display_name: "Almost Maximize",  keywords: &["almost","max"],                     frame: PresetFrame{x:0.025,y:0.025,w:0.95,h:0.95} },
    Preset { name: "center",           display_name: "Center",           keywords: &["center","middle"],                  frame: PresetFrame{x:0.15,y:0.10,w:0.70,h:0.80} },
    Preset { name: "center-large",     display_name: "Center Large",     keywords: &["center","large"],                   frame: PresetFrame{x:0.10,y:0.05,w:0.80,h:0.90} },
    Preset { name: "center-small",     display_name: "Center Small",     keywords: &["center","small"],                   frame: PresetFrame{x:0.25,y:0.20,w:0.50,h:0.60} },
    // Sixths (top/bottom row, 3 columns)
    Preset { name: "top-left-sixth",    display_name: "Top Left Sixth",    keywords: &["top","left","sixth"],    frame: PresetFrame{x:0.0,y:0.0,w:0.333,h:0.5} },
    Preset { name: "top-center-sixth",  display_name: "Top Center Sixth",  keywords: &["top","center","sixth"],  frame: PresetFrame{x:0.333,y:0.0,w:0.334,h:0.5} },
    Preset { name: "top-right-sixth",   display_name: "Top Right Sixth",   keywords: &["top","right","sixth"],   frame: PresetFrame{x:0.667,y:0.0,w:0.333,h:0.5} },
    Preset { name: "bottom-left-sixth", display_name: "Bottom Left Sixth", keywords: &["bottom","left","sixth"], frame: PresetFrame{x:0.0,y:0.5,w:0.333,h:0.5} },
    Preset { name: "bottom-center-sixth", display_name: "Bottom Center Sixth", keywords: &["bottom","center","sixth"], frame: PresetFrame{x:0.333,y:0.5,w:0.334,h:0.5} },
    Preset { name: "bottom-right-sixth", display_name: "Bottom Right Sixth", keywords: &["bottom","right","sixth"], frame: PresetFrame{x:0.667,y:0.5,w:0.333,h:0.5} },
    // Restore = sentinel (handled specially)
    Preset { name: "restore", display_name: "Restore", keywords: &["restore","undo"], frame: PresetFrame{x:0.0,y:0.0,w:0.0,h:0.0} },
];

pub fn lookup(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_frames_in_unit_range() {
        for p in PRESETS {
            if p.name == "restore" {
                continue;
            }
            assert!(p.frame.x >= 0.0 && p.frame.x <= 1.0, "{}", p.name);
            assert!(p.frame.w > 0.0 && p.frame.x + p.frame.w <= 1.001, "{}", p.name);
            assert!(p.frame.y >= 0.0 && p.frame.y <= 1.0, "{}", p.name);
            assert!(p.frame.h > 0.0 && p.frame.y + p.frame.h <= 1.001, "{}", p.name);
        }
    }
    #[test]
    fn lookup_known_preset() {
        assert!(lookup("left-half").is_some());
        assert!(lookup("nope").is_none());
    }
    #[test]
    fn count_is_25() {
        assert_eq!(PRESETS.len(), 26); // 25 + restore
    }
}
