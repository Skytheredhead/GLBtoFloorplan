use std::fmt::Write;

use super::{FloorplanDocument, Point2, Rect};

const SVG_W: f64 = 1000.0;
const SVG_H: f64 = 760.0;
const PAD: f64 = 94.0;

pub fn render_svg(plan: &FloorplanDocument) -> String {
    let mapper = Mapper::new(plan.width_ft, plan.depth_ft, SVG_W, SVG_H, PAD);
    let mut out = String::new();

    writeln!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {SVG_W} {SVG_H}" role="img" aria-label="Measured floorplan">"#
    )
    .unwrap();
    out.push_str(
        r#"<style>
        .title{font:700 24px Inter,Arial,sans-serif;fill:#17191f}
        .meta{font:500 12px Inter,Arial,sans-serif;fill:#555b66}
        .room-label{font:700 12px Inter,Arial,sans-serif;fill:#2b3038}
        .room-area{font:500 10px Inter,Arial,sans-serif;fill:#8a919c}
        .dim{font:600 10px Inter,Arial,sans-serif;fill:#343942}
        .wall{stroke:#55596b;stroke-linecap:square}
        .thin{stroke:#9ca3af;stroke-width:1.5;fill:none}
        .furn{stroke:#c8ced6;stroke-width:1.5;fill:#f7f8fa}
        </style>"#,
    );
    out.push_str(r##"<rect x="0" y="0" width="1000" height="760" fill="#fff"/>"##);
    out.push_str(r#"<text class="title" x="500" y="40" text-anchor="middle">Floorplan</text>"#);
    write!(
        out,
        r#"<text class="meta" x="500" y="60" text-anchor="middle">Approx. {:.0} sq ft • {:.0}% detection confidence</text>"#,
        plan.total_area_sqft,
        plan.confidence * 100.0
    )
    .unwrap();

    for room in &plan.rooms {
        let points = room
            .polygon
            .iter()
            .map(|point| {
                let (x, y) = mapper.pt(point);
                format!("{x:.2},{y:.2}")
            })
            .collect::<Vec<_>>()
            .join(" ");
        write!(
            out,
            r##"<polygon points="{points}" fill="{}" stroke="#edf0f3" stroke-width="1"/>"##,
            room.color
        )
        .unwrap();
    }

    for wall in &plan.walls {
        let (x1, y1) = mapper.pt(&wall.start);
        let (x2, y2) = mapper.pt(&wall.end);
        let stroke = (wall.thickness_ft * mapper.scale).max(6.0);
        write!(
            out,
            r#"<line class="wall" x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke-width="{stroke:.2}"/>"#
        )
        .unwrap();
    }

    for opening in &plan.openings {
        let (x1, y1) = mapper.pt(&opening.start);
        let (x2, y2) = mapper.pt(&opening.end);
        out.push_str(r#"<g>"#);
        write!(
            out,
            r##"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="#fff" stroke-width="9" stroke-linecap="square"/>"##
        )
        .unwrap();
        write!(
            out,
            r#"<line class="thin" x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}"/>"#
        )
        .unwrap();
        if opening.kind == "door" {
            let sweep = 42.0;
            write!(
                out,
                r#"<path class="thin" d="M {x1:.2} {y1:.2} Q {:.2} {:.2} {:.2} {:.2}"/>"#,
                x1 + sweep,
                y1 - sweep,
                x1 + sweep,
                y1
            )
            .unwrap();
        }
        out.push_str("</g>");
    }

    for item in &plan.furniture {
        let (x, y, w, h) = mapper.rect(&item.rect);
        let opacity = if item.confidence >= 0.7 { 1.0 } else { 0.55 };
        write!(
            out,
            r#"<rect class="furn" x="{x:.2}" y="{y:.2}" width="{w:.2}" height="{h:.2}" opacity="{opacity:.2}" rx="2"/>"#
        )
        .unwrap();
        if let Some(label) = &item.label {
            write!(
                out,
                r#"<text class="meta" x="{:.2}" y="{:.2}" text-anchor="middle">{}</text>"#,
                x + w / 2.0,
                y + h / 2.0,
                escape_xml(label)
            )
            .unwrap();
        }
    }

    for room in &plan.rooms {
        let (cx, cy) = centroid(&room.polygon);
        let point = Point2 { x: cx, y: cy };
        let (x, y) = mapper.pt(&point);
        write!(
            out,
            r#"<text class="room-label" x="{x:.2}" y="{y:.2}" text-anchor="middle">{}</text>"#,
            escape_xml(&room.label)
        )
        .unwrap();
        write!(
            out,
            r#"<text class="room-area" x="{x:.2}" y="{:.2}" text-anchor="middle">{:.0} sq ft</text>"#,
            y + 15.0,
            room.area_sqft
        )
        .unwrap();
    }

    for dim in &plan.dimensions {
        let (x1, y1) = mapper.pt_offset(&dim.start, dim.offset_ft);
        let (x2, y2) = mapper.pt_offset(&dim.end, dim.offset_ft);
        write!(
            out,
            r#"<line class="thin" x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}"/>"#
        )
        .unwrap();
        write!(
            out,
            r#"<text class="dim" x="{:.2}" y="{:.2}" text-anchor="middle">{}</text>"#,
            (x1 + x2) / 2.0,
            (y1 + y2) / 2.0 - 5.0,
            escape_xml(&dim.label)
        )
        .unwrap();
    }

    draw_scale_bar(&mut out);

    if !plan.warnings.is_empty() {
        write!(
            out,
            r#"<text class="meta" x="500" y="730" text-anchor="middle">{}</text>"#,
            escape_xml(&plan.warnings[0])
        )
        .unwrap();
    }

    out.push_str("</svg>");
    out
}

pub fn render_pdf(plan: &FloorplanDocument) -> Vec<u8> {
    let mapper = Mapper::new(plan.width_ft, plan.depth_ft, 792.0, 612.0, 78.0);
    let mut content = String::new();
    content.push_str("1 1 1 rg 0 0 792 612 re f\n");
    text(&mut content, 396.0, 580.0, 18.0, "Floorplan", true);
    text(
        &mut content,
        396.0,
        562.0,
        9.0,
        &format!(
            "Approx. {:.0} sq ft - {:.0}% detection confidence",
            plan.total_area_sqft,
            plan.confidence * 100.0
        ),
        true,
    );

    for room in &plan.rooms {
        rgb(&mut content, &room.color, false);
        path_polygon(&mut content, &mapper, &room.polygon, "f");
    }

    content.push_str("0.33 0.35 0.42 RG\n");
    for wall in &plan.walls {
        let (x1, y1) = mapper.pdf_pt(&wall.start);
        let (x2, y2) = mapper.pdf_pt(&wall.end);
        let stroke = (wall.thickness_ft * mapper.scale).max(5.0);
        writeln!(
            content,
            "{stroke:.2} w {x1:.2} {y1:.2} m {x2:.2} {y2:.2} l S"
        )
        .unwrap();
    }

    content.push_str("1 1 1 RG\n");
    for opening in &plan.openings {
        let (x1, y1) = mapper.pdf_pt(&opening.start);
        let (x2, y2) = mapper.pdf_pt(&opening.end);
        writeln!(content, "8 w {x1:.2} {y1:.2} m {x2:.2} {y2:.2} l S").unwrap();
    }

    content.push_str("0.78 0.80 0.84 RG 0.96 0.97 0.98 rg\n");
    for item in &plan.furniture {
        let (x, y, w, h) = mapper.pdf_rect(&item.rect);
        writeln!(content, "1 w {x:.2} {y:.2} {w:.2} {h:.2} re B").unwrap();
        if let Some(label) = &item.label {
            text(&mut content, x + w / 2.0, y + h / 2.0, 7.0, label, true);
        }
    }

    content.push_str("0.12 0.13 0.16 rg\n");
    for room in &plan.rooms {
        let (cx, cy) = centroid(&room.polygon);
        let (x, y) = mapper.pdf_pt(&Point2 { x: cx, y: cy });
        text(&mut content, x, y + 4.0, 8.5, &room.label, true);
        text(
            &mut content,
            x,
            y - 7.0,
            7.0,
            &format!("{:.0} sq ft", room.area_sqft),
            true,
        );
    }

    content.push_str("0.2 0.22 0.26 RG\n");
    for dim in &plan.dimensions {
        let (x1, y1) = mapper.pdf_pt_offset(&dim.start, dim.offset_ft);
        let (x2, y2) = mapper.pdf_pt_offset(&dim.end, dim.offset_ft);
        writeln!(content, "0.7 w {x1:.2} {y1:.2} m {x2:.2} {y2:.2} l S").unwrap();
        text(
            &mut content,
            (x1 + x2) / 2.0,
            (y1 + y2) / 2.0 + 5.0,
            7.0,
            &dim.label,
            true,
        );
    }

    make_pdf(content.into_bytes())
}

fn make_pdf(content: Vec<u8>) -> Vec<u8> {
    let mut pdf = Vec::<u8>::new();
    let mut offsets = Vec::<usize>::new();
    pdf.extend_from_slice(b"%PDF-1.4\n");
    write_obj(
        &mut pdf,
        &mut offsets,
        1,
        b"<< /Type /Catalog /Pages 2 0 R >>",
    );
    write_obj(
        &mut pdf,
        &mut offsets,
        2,
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    );
    write_obj(
        &mut pdf,
        &mut offsets,
        3,
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 792 612] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>",
    );
    write_obj(
        &mut pdf,
        &mut offsets,
        4,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    );
    offsets.push(pdf.len());
    pdf.extend_from_slice(format!("5 0 obj\n<< /Length {} >>\nstream\n", content.len()).as_bytes());
    pdf.extend_from_slice(&content);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");
    let xref = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes(),
    );
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    pdf
}

fn write_obj(pdf: &mut Vec<u8>, offsets: &mut Vec<usize>, id: usize, body: &[u8]) {
    offsets.push(pdf.len());
    pdf.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
    pdf.extend_from_slice(body);
    pdf.extend_from_slice(b"\nendobj\n");
}

fn path_polygon(content: &mut String, mapper: &Mapper, points: &[Point2], op: &str) {
    if let Some(first) = points.first() {
        let (x, y) = mapper.pdf_pt(first);
        writeln!(content, "{x:.2} {y:.2} m").unwrap();
        for point in &points[1..] {
            let (x, y) = mapper.pdf_pt(point);
            writeln!(content, "{x:.2} {y:.2} l").unwrap();
        }
        writeln!(content, "h {op}").unwrap();
    }
}

fn rgb(content: &mut String, color: &str, stroke: bool) {
    let color = color.trim_start_matches('#');
    if color.len() == 6 {
        let r = u8::from_str_radix(&color[0..2], 16).unwrap_or(255) as f64 / 255.0;
        let g = u8::from_str_radix(&color[2..4], 16).unwrap_or(255) as f64 / 255.0;
        let b = u8::from_str_radix(&color[4..6], 16).unwrap_or(255) as f64 / 255.0;
        if stroke {
            writeln!(content, "{r:.3} {g:.3} {b:.3} RG").unwrap();
        } else {
            writeln!(content, "{r:.3} {g:.3} {b:.3} rg").unwrap();
        }
    }
}

fn text(content: &mut String, x: f64, y: f64, size: f64, value: &str, centered: bool) {
    let escaped = pdf_escape(value);
    let tx = if centered {
        x - (value.chars().count() as f64 * size * 0.24)
    } else {
        x
    };
    writeln!(
        content,
        "BT /F1 {size:.2} Tf {tx:.2} {y:.2} Td ({escaped}) Tj ET"
    )
    .unwrap();
}

fn draw_scale_bar(out: &mut String) {
    out.push_str(
        r##"<line x1="70" y1="710" x2="170" y2="710" stroke="#111827" stroke-width="5"/>"##,
    );
    out.push_str(r#"<text class="dim" x="70" y="700">0</text>"#);
    out.push_str(r#"<text class="dim" x="170" y="700" text-anchor="middle">3 m</text>"#);
}

fn centroid(points: &[Point2]) -> (f64, f64) {
    let len = points.len().max(1) as f64;
    let x = points.iter().map(|p| p.x).sum::<f64>() / len;
    let y = points.iter().map(|p| p.y).sum::<f64>() / len;
    (x, y)
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn pdf_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
        .replace('\n', " ")
        .replace('\r', " ")
}

struct Mapper {
    depth_ft: f64,
    canvas_h: f64,
    pad: f64,
    scale: f64,
}

impl Mapper {
    fn new(width_ft: f64, depth_ft: f64, canvas_w: f64, canvas_h: f64, pad: f64) -> Self {
        let scale = ((canvas_w - pad * 2.0) / width_ft).min((canvas_h - pad * 2.0) / depth_ft);
        Self {
            depth_ft,
            canvas_h,
            pad,
            scale,
        }
    }

    fn pt(&self, point: &Point2) -> (f64, f64) {
        (
            self.pad + point.x * self.scale,
            self.pad + point.y * self.scale,
        )
    }

    fn pdf_pt(&self, point: &Point2) -> (f64, f64) {
        let (x, y) = self.pt(point);
        (x, self.canvas_h - y)
    }

    fn pt_offset(&self, point: &Point2, offset_ft: f64) -> (f64, f64) {
        let mut p = Point2 {
            x: point.x,
            y: point.y,
        };
        if point.y.abs() < 0.01 || (point.y - self.depth_ft).abs() < 0.01 {
            p.y += offset_ft;
        } else {
            p.x += offset_ft;
        }
        self.pt(&p)
    }

    fn pdf_pt_offset(&self, point: &Point2, offset_ft: f64) -> (f64, f64) {
        let (x, y) = self.pt_offset(point, offset_ft);
        (x, self.canvas_h - y)
    }

    fn rect(&self, rect: &Rect) -> (f64, f64, f64, f64) {
        let (x, y) = self.pt(&Point2 {
            x: rect.x,
            y: rect.y,
        });
        (x, y, rect.w * self.scale, rect.h * self.scale)
    }

    fn pdf_rect(&self, rect: &Rect) -> (f64, f64, f64, f64) {
        let (x, y, w, h) = self.rect(rect);
        (x, self.canvas_h - y - h, w, h)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use uuid::Uuid;

    use crate::processing::{geometry::build_floorplan, gltf_import::ScanSummary};

    use super::*;

    #[test]
    fn svg_and_pdf_are_generated() {
        let plan = build_floorplan(
            Uuid::nil(),
            ScanSummary {
                width_m: 9.0,
                depth_m: 7.0,
                height_m: 2.8,
                vertex_count: 8,
                semantic_hints: BTreeSet::new(),
                warnings: vec![],
            },
        )
        .unwrap();
        let svg = render_svg(&plan);
        let pdf = render_pdf(&plan);
        assert!(svg.contains("Floorplan"));
        assert!(svg.contains("sq ft"));
        assert!(pdf.starts_with(b"%PDF-1.4"));
    }
}
