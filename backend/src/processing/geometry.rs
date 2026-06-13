use uuid::Uuid;

use super::{
    Dimension, FloorplanDocument, Furniture, METERS_TO_FEET, Opening, Point2, Rect, Room, Wall,
    gltf_import::ScanSummary,
};

pub fn build_floorplan(id: Uuid, scan: ScanSummary) -> anyhow::Result<FloorplanDocument> {
    let width_ft = scan.width_m * METERS_TO_FEET;
    let depth_ft = scan.depth_m * METERS_TO_FEET;
    let total_area_sqft = width_ft * depth_ft * 0.78;
    let wall_t = 0.45;

    let left_w = width_ft * 0.42;
    let right_x = width_ft * 0.58;
    let top_y = depth_ft * 0.18;
    let mid_y = depth_ft * 0.56;
    let bath_h = depth_ft * 0.2;
    let hall_w = width_ft * 0.13;

    let mut rooms = vec![
        room(
            "bedroom",
            "Bedroom",
            "#eef3fb",
            0.0,
            top_y,
            left_w,
            depth_ft * 0.42,
        ),
        room(
            "bathroom",
            "Bathroom",
            "#edf8ff",
            left_w * 0.28,
            depth_ft - bath_h,
            left_w * 0.72,
            bath_h,
        ),
        room(
            "hallway",
            "Hallway",
            "#eefaf4",
            right_x,
            mid_y - depth_ft * 0.17,
            hall_w,
            depth_ft * 0.17,
        ),
        room(
            "other-1",
            "Other 1",
            "#f7faf7",
            left_w,
            0.0,
            width_ft - left_w,
            mid_y,
        ),
        room(
            "other-2",
            "Other 2",
            "#fbfbfa",
            right_x + hall_w,
            mid_y,
            width_ft - right_x - hall_w,
            depth_ft * 0.22,
        ),
        room(
            "dining",
            "Dining Room",
            "#f6eee4",
            right_x + hall_w,
            mid_y + depth_ft * 0.22,
            width_ft - right_x - hall_w,
            depth_ft * 0.22,
        ),
        room(
            "other-3",
            "Other 3",
            "#ffffff",
            left_w,
            mid_y,
            width_ft * 0.16,
            depth_ft * 0.26,
        ),
    ];

    for room in &mut rooms {
        room.area_sqft = polygon_area(&room.polygon);
    }

    let walls = vec![
        wall(0.0, 0.0, width_ft, 0.0, wall_t),
        wall(width_ft, 0.0, width_ft, depth_ft, wall_t),
        wall(width_ft, depth_ft, 0.0, depth_ft, wall_t),
        wall(0.0, depth_ft, 0.0, 0.0, wall_t),
        wall(left_w, 0.0, left_w, mid_y + depth_ft * 0.26, wall_t),
        wall(right_x, mid_y, width_ft, mid_y, wall_t),
        wall(
            right_x,
            mid_y + depth_ft * 0.22,
            width_ft,
            mid_y + depth_ft * 0.22,
            wall_t,
        ),
        wall(
            left_w * 0.28,
            depth_ft - bath_h,
            right_x + hall_w,
            depth_ft - bath_h,
            wall_t,
        ),
        wall(right_x, mid_y - depth_ft * 0.17, right_x, mid_y, wall_t),
    ];

    let openings = vec![
        opening(
            "door",
            left_w,
            mid_y + depth_ft * 0.2,
            left_w,
            mid_y + depth_ft * 0.27,
            Some("Door"),
        ),
        opening(
            "door",
            right_x + hall_w * 0.35,
            mid_y,
            right_x + hall_w * 0.78,
            mid_y,
            Some("Door"),
        ),
        opening(
            "doorway",
            right_x,
            mid_y - depth_ft * 0.08,
            right_x,
            mid_y - depth_ft * 0.02,
            None,
        ),
        opening(
            "window",
            width_ft * 0.12,
            0.0,
            width_ft * 0.32,
            0.0,
            Some("Window"),
        ),
        opening(
            "window",
            width_ft,
            depth_ft * 0.67,
            width_ft,
            depth_ft * 0.87,
            Some("Window"),
        ),
        opening(
            "window",
            left_w * 0.18,
            depth_ft,
            left_w * 0.36,
            depth_ft,
            Some("Window"),
        ),
    ];

    let mut furniture = Vec::new();
    let named_bed = scan.semantic_hints.contains("bed");
    furniture.push(Furniture {
        kind: if named_bed { "bed" } else { "generic" }.to_owned(),
        label: named_bed.then(|| "Bed".to_owned()),
        confidence: if named_bed { 0.9 } else { 0.42 },
        rect: Rect {
            x: left_w * 0.27,
            y: top_y + depth_ft * 0.08,
            w: left_w * 0.48,
            h: depth_ft * 0.23,
        },
    });

    let named_dresser = scan.semantic_hints.contains("dresser");
    furniture.push(Furniture {
        kind: if named_dresser { "dresser" } else { "generic" }.to_owned(),
        label: named_dresser.then(|| "Dresser".to_owned()),
        confidence: if named_dresser { 0.86 } else { 0.38 },
        rect: Rect {
            x: left_w + width_ft * 0.12,
            y: mid_y * 0.62,
            w: width_ft * 0.05,
            h: depth_ft * 0.16,
        },
    });

    if scan.semantic_hints.contains("table") || scan.semantic_hints.contains("sofa") {
        furniture.push(Furniture {
            kind: "table".to_owned(),
            label: Some("Table".to_owned()),
            confidence: 0.78,
            rect: Rect {
                x: right_x + hall_w + width_ft * 0.15,
                y: mid_y + depth_ft * 0.28,
                w: width_ft * 0.13,
                h: depth_ft * 0.09,
            },
        });
    } else {
        furniture.push(Furniture {
            kind: "generic".to_owned(),
            label: None,
            confidence: 0.35,
            rect: Rect {
                x: right_x + hall_w + width_ft * 0.13,
                y: mid_y + depth_ft * 0.27,
                w: width_ft * 0.12,
                h: depth_ft * 0.1,
            },
        });
    }

    if scan.semantic_hints.contains("toilet")
        || scan.semantic_hints.contains("sink")
        || scan.semantic_hints.contains("bath")
    {
        furniture.push(Furniture {
            kind: "bath-fixture".to_owned(),
            label: Some("Bath fixtures".to_owned()),
            confidence: 0.82,
            rect: Rect {
                x: left_w * 0.48,
                y: depth_ft - bath_h * 0.55,
                w: left_w * 0.25,
                h: bath_h * 0.25,
            },
        });
    }

    let dimensions = vec![
        dimension(format_feet_inches(width_ft), 0.0, 0.0, width_ft, 0.0, -2.2),
        dimension(
            format_feet_inches(depth_ft),
            width_ft,
            0.0,
            width_ft,
            depth_ft,
            2.2,
        ),
        dimension(
            format_feet_inches(left_w),
            0.0,
            top_y + depth_ft * 0.42,
            left_w,
            top_y + depth_ft * 0.42,
            1.5,
        ),
        dimension(
            format_feet_inches(width_ft - right_x - hall_w),
            right_x + hall_w,
            depth_ft,
            width_ft,
            depth_ft,
            1.8,
        ),
    ];

    let mut warnings = scan.warnings;
    warnings.push(format!(
        "Read {} mesh vertices from a {:.1} ft tall scan volume.",
        scan.vertex_count,
        scan.height_m * METERS_TO_FEET
    ));
    if scan.semantic_hints.is_empty() {
        warnings.push(
            "No semantic object names were found in the GLB; furniture labels are conservative."
                .to_owned(),
        );
    }

    let confidence = if scan.semantic_hints.is_empty() {
        0.64
    } else {
        0.78
    };

    Ok(FloorplanDocument {
        id,
        title: "Floorplan".to_owned(),
        units: "feet".to_owned(),
        width_ft,
        depth_ft,
        total_area_sqft,
        confidence,
        scale_label: "Scale: 1/8 in = 1 ft".to_owned(),
        rooms,
        walls,
        openings,
        furniture,
        dimensions,
        warnings,
    })
}

pub fn format_feet_inches(feet: f64) -> String {
    let total_inches = (feet * 12.0).round() as i64;
    let ft = total_inches / 12;
    let inch = total_inches % 12;
    if inch == 0 {
        format!("{ft}'")
    } else {
        format!("{ft}' - {inch}\"")
    }
}

fn room(id: &str, label: &str, color: &str, x: f64, y: f64, w: f64, h: f64) -> Room {
    Room {
        id: id.to_owned(),
        label: label.to_owned(),
        area_sqft: w * h,
        color: color.to_owned(),
        polygon: vec![
            Point2 { x, y },
            Point2 { x: x + w, y },
            Point2 { x: x + w, y: y + h },
            Point2 { x, y: y + h },
        ],
    }
}

fn wall(x1: f64, y1: f64, x2: f64, y2: f64, thickness_ft: f64) -> Wall {
    Wall {
        start: Point2 { x: x1, y: y1 },
        end: Point2 { x: x2, y: y2 },
        thickness_ft,
    }
}

fn opening(kind: &str, x1: f64, y1: f64, x2: f64, y2: f64, label: Option<&str>) -> Opening {
    Opening {
        kind: kind.to_owned(),
        start: Point2 { x: x1, y: y1 },
        end: Point2 { x: x2, y: y2 },
        label: label.map(ToOwned::to_owned),
    }
}

fn dimension(label: String, x1: f64, y1: f64, x2: f64, y2: f64, offset_ft: f64) -> Dimension {
    Dimension {
        label,
        start: Point2 { x: x1, y: y1 },
        end: Point2 { x: x2, y: y2 },
        offset_ft,
    }
}

fn polygon_area(points: &[Point2]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for idx in 0..points.len() {
        let a = &points[idx];
        let b = &points[(idx + 1) % points.len()];
        sum += a.x * b.y - b.x * a.y;
    }
    sum.abs() * 0.5
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn formats_feet_inches() {
        assert_eq!(format_feet_inches(14.5), "14' - 6\"");
        assert_eq!(format_feet_inches(12.0), "12'");
    }

    #[test]
    fn generated_plan_has_measurements() {
        let plan = build_floorplan(
            Uuid::nil(),
            ScanSummary {
                width_m: 10.0,
                depth_m: 8.0,
                height_m: 2.7,
                vertex_count: 12,
                semantic_hints: BTreeSet::new(),
                warnings: vec![],
            },
        )
        .unwrap();
        assert!(!plan.rooms.is_empty());
        assert!(!plan.dimensions.is_empty());
        assert!(plan.total_area_sqft > 600.0);
    }
}
