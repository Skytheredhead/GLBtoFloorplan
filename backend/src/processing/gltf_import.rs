use std::collections::BTreeSet;

use nalgebra::{Matrix4, Point3, Quaternion, Translation3, UnitQuaternion, Vector3, Vector4};

#[derive(Debug, Clone)]
pub struct ScanSummary {
    pub width_m: f64,
    pub depth_m: f64,
    pub height_m: f64,
    pub vertex_count: usize,
    pub semantic_hints: BTreeSet<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct Bounds {
    min: Vector3<f64>,
    max: Vector3<f64>,
    count: usize,
}

impl Bounds {
    fn new() -> Self {
        Self {
            min: Vector3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY),
            max: Vector3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
            count: 0,
        }
    }

    fn push(&mut self, point: Point3<f64>) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.min.z = self.min.z.min(point.z);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
        self.max.z = self.max.z.max(point.z);
        self.count += 1;
    }
}

pub fn scan_glb(bytes: &[u8]) -> anyhow::Result<ScanSummary> {
    let (document, buffers, _) = gltf::import_slice(bytes)?;
    let mut bounds = Bounds::new();
    let mut hints = BTreeSet::new();

    for scene in document.scenes() {
        for node in scene.nodes() {
            visit_node(node, Matrix4::identity(), &buffers, &mut bounds, &mut hints);
        }
    }

    if bounds.count == 0 {
        anyhow::bail!("GLB did not contain mesh positions");
    }

    let width_m = bounds.max.x - bounds.min.x;
    let depth_m = bounds.max.z - bounds.min.z;
    let height_m = bounds.max.y - bounds.min.y;
    let mut warnings = Vec::new();

    if width_m < 1.0 || depth_m < 1.0 {
        anyhow::bail!(
            "GLB scale is too small or missing real-world units; auto-only measurement mode cannot export credible dimensions"
        );
    }
    if width_m > 120.0 || depth_m > 120.0 || height_m > 30.0 {
        anyhow::bail!(
            "GLB scale is implausibly large for a residential floorplan; auto-only measurement mode cannot export credible dimensions"
        );
    }
    if height_m < 1.5 {
        warnings.push("The model height is low for a room scan; wall and opening detection may be incomplete.".to_owned());
    }

    Ok(ScanSummary {
        width_m,
        depth_m,
        height_m,
        vertex_count: bounds.count,
        semantic_hints: hints,
        warnings,
    })
}

fn visit_node(
    node: gltf::Node<'_>,
    parent_transform: Matrix4<f64>,
    buffers: &[gltf::buffer::Data],
    bounds: &mut Bounds,
    hints: &mut BTreeSet<String>,
) {
    if let Some(name) = node.name() {
        collect_hints(name, hints);
    }

    let transform = parent_transform * node_transform(&node);

    if let Some(mesh) = node.mesh() {
        if let Some(name) = mesh.name() {
            collect_hints(name, hints);
        }

        for primitive in mesh.primitives() {
            if let Some(name) = primitive.material().name() {
                collect_hints(name, hints);
            }

            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()].0));
            if let Some(positions) = reader.read_positions() {
                for position in positions {
                    let point = transform_point(
                        &transform,
                        Point3::new(position[0] as f64, position[1] as f64, position[2] as f64),
                    );
                    bounds.push(point);
                }
            }
        }
    }

    for child in node.children() {
        visit_node(child, transform, buffers, bounds, hints);
    }
}

fn node_transform(node: &gltf::Node<'_>) -> Matrix4<f64> {
    let (translation, rotation, scale) = node.transform().decomposed();
    let t = Translation3::new(
        translation[0] as f64,
        translation[1] as f64,
        translation[2] as f64,
    )
    .to_homogeneous();
    let r = UnitQuaternion::from_quaternion(Quaternion::new(
        rotation[3] as f64,
        rotation[0] as f64,
        rotation[1] as f64,
        rotation[2] as f64,
    ))
    .to_homogeneous();
    let s = Matrix4::new_nonuniform_scaling(&Vector3::new(
        scale[0] as f64,
        scale[1] as f64,
        scale[2] as f64,
    ));
    t * r * s
}

fn transform_point(transform: &Matrix4<f64>, point: Point3<f64>) -> Point3<f64> {
    let raw = transform * Vector4::new(point.x, point.y, point.z, 1.0);
    let w = if raw.w.abs() < f64::EPSILON {
        1.0
    } else {
        raw.w
    };
    Point3::new(raw.x / w, raw.y / w, raw.z / w)
}

fn collect_hints(name: &str, hints: &mut BTreeSet<String>) {
    let lower = name.to_ascii_lowercase();
    for hint in [
        "bed", "dresser", "door", "window", "sofa", "couch", "table", "toilet", "sink", "bath",
        "shower", "closet",
    ] {
        if lower.contains(hint) {
            hints.insert(hint.to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_semantic_hints_case_insensitively() {
        let mut hints = BTreeSet::new();
        collect_hints("Primary_Bed_and_Dresser", &mut hints);
        assert!(hints.contains("bed"));
        assert!(hints.contains("dresser"));
    }
}
