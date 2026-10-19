//! CPU-viewable meshes and portable 3D exports. Native CAD tessellation is supplied
//! by the packaged KiCad/OpenCascade backend; GLB/STL/OBJ import is self-contained.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::path::Path;
pub type V3 = [f64; 3];
#[derive(Clone, Debug)]
pub struct Triangle {
    pub vertices: [V3; 3],
    pub color: [u8; 3],
    /// Imported smooth normals, in the same Z-up coordinate system as vertices.
    pub normals: Option<[V3; 3]>,
    pub metallic: f64,
    pub roughness: f64,
    pub object: Option<String>,
}
#[derive(Clone, Default, Debug)]
pub struct Mesh {
    pub triangles: Vec<Triangle>,
    pub warnings: Vec<String>,
}
impl Mesh {
    pub fn bounds(&self) -> (V3, V3) {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for t in &self.triangles {
            for v in t.vertices {
                for i in 0..3 {
                    lo[i] = lo[i].min(v[i]);
                    hi[i] = hi[i].max(v[i]);
                }
            }
        }
        if !lo[0].is_finite() {
            return ([-1.; 3], [1.; 3]);
        }
        (lo, hi)
    }
    pub fn info(&self) -> Value {
        json!({"triangle_count":self.triangles.len(),"bounds_mm":self.bounds(),"up_axis":"Z","units":"mm","warnings":self.warnings})
    }
    pub fn read(path: &Path) -> Result<Self> {
        let bytes =
            std::fs::read(path).with_context(|| format!("Read 3D model {}", path.display()))?;
        ensure!(bytes.len() <= 256 * 1024 * 1024, "3D mesh exceeds 256 MiB");
        match path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str()
        {
            "glb" => Self::glb(&bytes),
            "stl" => Self::stl(&bytes),
            "obj" => Self::obj(std::str::from_utf8(&bytes)?),
            _ => anyhow::bail!(
                "This model needs CAD conversion through the KiCad backend: {}",
                path.display()
            ),
        }
    }
    pub fn box_solid(&mut self, lo: V3, hi: V3, color: [u8; 3], object: Option<String>) {
        let v = [
            [lo[0], lo[1], lo[2]],
            [hi[0], lo[1], lo[2]],
            [hi[0], hi[1], lo[2]],
            [lo[0], hi[1], lo[2]],
            [lo[0], lo[1], hi[2]],
            [hi[0], lo[1], hi[2]],
            [hi[0], hi[1], hi[2]],
            [lo[0], hi[1], hi[2]],
        ];
        for f in [
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ] {
            self.triangles.push(Triangle {
                vertices: [v[f[0]], v[f[1]], v[f[2]]],
                color,
                normals: None,
                metallic: 0.,
                roughness: 0.7,
                object: object.clone(),
            });
        }
    }
    pub fn board_basic(board: &crate::pcb::Board) -> Self {
        let mut m = Self::default();
        let (lo, hi) = board.bounds();
        m.box_solid(
            [lo.0, -hi.1, -board.thickness()],
            [hi.0, -lo.1, 0.],
            [30, 112, 65],
            Some("board".into()),
        );
        for p in board.pads() {
            let z = if p.layers.iter().any(|l| l == "F.Cu" || l == "*.Cu") {
                0.02
            } else {
                -board.thickness() - 0.08
            };
            let half = (p.size.0 / 2., p.size.1 / 2.);
            m.box_solid(
                [p.at.0 - half.0, -p.at.1 - half.1, z],
                [p.at.0 + half.0, -p.at.1 + half.1, z + 0.08],
                [198, 159, 64],
                Some(p.reference),
            );
        }
        m.warnings.push("Basic board preview: exact outlines, holes and component models are loaded by the native 3D backend".into());
        m
    }
    pub fn stl(bytes: &[u8]) -> Result<Self> {
        let mut m = Self::default();
        let binary = if bytes.len() >= 84 {
            let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
            count
                .checked_mul(50)
                .and_then(|n| n.checked_add(84))
                .is_some_and(|n| n == bytes.len())
        } else {
            false
        };
        if binary {
            let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
            ensure!(count <= 2_000_000, "STL exceeds two million triangles");
            for i in 0..count {
                let start = 84 + i * 50 + 12;
                let mut v = [[0.; 3]; 3];
                for (j, vertex) in v.iter_mut().enumerate() {
                    for (k, coordinate) in vertex.iter_mut().enumerate() {
                        let off = start + (j * 3 + k) * 4;
                        *coordinate =
                            f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap()) as f64;
                    }
                }
                m.triangles.push(Triangle {
                    vertices: v,
                    color: [145, 150, 155],
                    normals: None,
                    metallic: 0.,
                    roughness: 0.7,
                    object: None,
                });
            }
        } else {
            let text = std::str::from_utf8(bytes)?;
            let mut vertices = vec![];
            for line in text.lines() {
                let fields: Vec<_> = line.split_whitespace().collect();
                if fields.first() == Some(&"vertex") {
                    ensure!(fields.len() == 4, "Malformed STL vertex");
                    vertices.push([fields[1].parse()?, fields[2].parse()?, fields[3].parse()?]);
                }
            }
            ensure!(
                vertices.len() % 3 == 0 && !vertices.is_empty(),
                "STL has no valid triangular facets"
            );
            for v in vertices.chunks(3) {
                m.triangles.push(Triangle {
                    vertices: [v[0], v[1], v[2]],
                    color: [145, 150, 155],
                    normals: None,
                    metallic: 0.,
                    roughness: 0.7,
                    object: None,
                });
            }
        }
        ensure!(
            m.triangles
                .iter()
                .all(|t| t.vertices.iter().flatten().all(|v| v.is_finite())),
            "Nonfinite STL vertices"
        );
        Ok(m)
    }
    pub fn obj(text: &str) -> Result<Self> {
        let mut m = Self::default();
        let mut vertices: Vec<V3> = vec![];
        for line in text.lines() {
            let f: Vec<_> = line.split_whitespace().collect();
            if f.first() == Some(&"v") && f.len() >= 4 {
                vertices.push([f[1].parse()?, f[2].parse()?, f[3].parse()?]);
            }
            if f.first() == Some(&"f") && f.len() >= 4 {
                let idx: Vec<_> = f[1..]
                    .iter()
                    .map(|v| {
                        let n = v.split('/').next().unwrap().parse::<i64>()?;
                        let i = if n > 0 {
                            n - 1
                        } else {
                            vertices.len() as i64 + n
                        };
                        ensure!(
                            i >= 0 && (i as usize) < vertices.len(),
                            "OBJ index is outside its vertex list"
                        );
                        Ok(i as usize)
                    })
                    .collect::<Result<_>>()?;
                for j in 1..idx.len() - 1 {
                    m.triangles.push(Triangle {
                        vertices: [vertices[idx[0]], vertices[idx[j]], vertices[idx[j + 1]]],
                        color: [145, 150, 155],
                        normals: None,
                        metallic: 0.,
                        roughness: 0.7,
                        object: None,
                    });
                }
            }
        }
        ensure!(!m.triangles.is_empty(), "OBJ contains no faces");
        Ok(m)
    }
    pub fn glb(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() >= 20 && &bytes[..4] == b"glTF",
            "Not a GLB file"
        );
        ensure!(
            u32::from_le_bytes(bytes[4..8].try_into().unwrap()) == 2,
            "Only GLB 2 is supported"
        );
        ensure!(
            u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize == bytes.len(),
            "Truncated GLB"
        );
        let mut pos = 12;
        let mut root = Value::Null;
        let mut binary: &[u8] = &[];
        while pos + 8 <= bytes.len() {
            let len = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
            let kind = &bytes[pos + 4..pos + 8];
            pos += 8;
            ensure!(pos + len <= bytes.len(), "Truncated GLB chunk");
            if kind == b"JSON" {
                root = serde_json::from_slice(&bytes[pos..pos + len])?;
            } else if kind == b"BIN\0" {
                binary = &bytes[pos..pos + len];
            }
            pos += len;
        }
        ensure!(
            !root.is_null() && !binary.is_empty(),
            "GLB needs embedded JSON and binary buffers"
        );
        fn accessor(root: &Value, bin: &[u8], id: usize) -> Result<Vec<Vec<f64>>> {
            let a = &root["accessors"][id];
            let view = &root["bufferViews"][a["bufferView"]
                .as_u64()
                .context("Sparse GLB accessors need conversion")?
                as usize];
            ensure!(
                view["buffer"].as_u64().unwrap_or(0) == 0,
                "External GLB buffers are unsupported"
            );
            let kind = a["componentType"]
                .as_u64()
                .context("Missing GLB component type")?;
            let size = match kind {
                5126 | 5125 => 4,
                5123 | 5122 => 2,
                5121 | 5120 => 1,
                _ => anyhow::bail!("Unsupported GLB component type {kind}"),
            };
            let components = match a["type"].as_str().unwrap_or("") {
                "SCALAR" => 1,
                "VEC2" => 2,
                "VEC3" => 3,
                "VEC4" => 4,
                _ => anyhow::bail!("Unsupported GLB accessor dimensions"),
            };
            let stride = view["byteStride"]
                .as_u64()
                .unwrap_or((components * size) as u64) as usize;
            let start = (view["byteOffset"].as_u64().unwrap_or(0)
                + a["byteOffset"].as_u64().unwrap_or(0)) as usize;
            let count = a["count"].as_u64().context("GLB accessor needs count")? as usize;
            ensure!(
                count <= 6_000_000 && stride >= size * components,
                "Invalid GLB accessor size"
            );
            let view_end = view["byteOffset"].as_u64().unwrap_or(0) as usize
                + view["byteLength"]
                    .as_u64()
                    .context("GLB view needs length")? as usize;
            let mut rows = vec![];
            for i in 0..count {
                let at = start + i * stride;
                ensure!(
                    at + size * components <= bin.len() && at + size * components <= view_end,
                    "GLB accessor exceeds buffer"
                );
                let mut row = vec![];
                for j in 0..components {
                    let p = at + j * size;
                    let n = match kind {
                        5126 => f32::from_le_bytes(bin[p..p + 4].try_into().unwrap()) as f64,
                        5125 => u32::from_le_bytes(bin[p..p + 4].try_into().unwrap()) as f64,
                        5123 => u16::from_le_bytes(bin[p..p + 2].try_into().unwrap()) as f64,
                        5122 => i16::from_le_bytes(bin[p..p + 2].try_into().unwrap()) as f64,
                        5121 => bin[p] as f64,
                        _ => (bin[p] as i8) as f64,
                    };
                    ensure!(n.is_finite(), "Nonfinite GLB vertex");
                    row.push(n);
                }
                rows.push(row);
            }
            Ok(rows)
        }
        fn mul(a: [f64; 16], b: [f64; 16]) -> [f64; 16] {
            let mut c = [0.; 16];
            for i in 0..4 {
                for j in 0..4 {
                    for k in 0..4 {
                        c[j * 4 + i] += a[k * 4 + i] * b[j * 4 + k];
                    }
                }
            }
            c
        }
        fn matrix(n: &Value) -> [f64; 16] {
            if let Some(m) = n["matrix"].as_array().filter(|m| m.len() == 16) {
                return std::array::from_fn(|i| m[i].as_f64().unwrap_or(0.));
            }
            let get = |key: &str, i: usize, d: f64| n[key][i].as_f64().unwrap_or(d);
            let x = get("rotation", 0, 0.);
            let y = get("rotation", 1, 0.);
            let z = get("rotation", 2, 0.);
            let w = get("rotation", 3, 1.);
            let s = [
                get("scale", 0, 1.),
                get("scale", 1, 1.),
                get("scale", 2, 1.),
            ];
            [
                (1. - 2. * y * y - 2. * z * z) * s[0],
                (2. * x * y + 2. * w * z) * s[0],
                (2. * x * z - 2. * w * y) * s[0],
                0.,
                (2. * x * y - 2. * w * z) * s[1],
                (1. - 2. * x * x - 2. * z * z) * s[1],
                (2. * y * z + 2. * w * x) * s[1],
                0.,
                (2. * x * z + 2. * w * y) * s[2],
                (2. * y * z - 2. * w * x) * s[2],
                (1. - 2. * x * x - 2. * y * y) * s[2],
                0.,
                get("translation", 0, 0.),
                get("translation", 1, 0.),
                get("translation", 2, 0.),
                1.,
            ]
        }
        fn walk(
            root: &Value,
            bin: &[u8],
            id: usize,
            parent: [f64; 16],
            depth: usize,
            m: &mut Mesh,
        ) -> Result<()> {
            ensure!(depth < 128, "GLB hierarchy is too deep");
            let n = &root["nodes"][id];
            let mat = mul(parent, matrix(n));
            if let Some(mesh) = n["mesh"].as_u64() {
                for p in root["meshes"][mesh as usize]["primitives"]
                    .as_array()
                    .context("GLB mesh needs primitives")?
                {
                    ensure!(
                        p["mode"].as_u64().unwrap_or(4) == 4,
                        "Convert GLB triangle strips/lines to triangles first"
                    );
                    let positions = accessor(
                        root,
                        bin,
                        p["attributes"]["POSITION"]
                            .as_u64()
                            .context("GLB primitive needs POSITION")?
                            as usize,
                    )?;
                    let indices = if let Some(idx) = p["indices"].as_u64() {
                        accessor(root, bin, idx as usize)?
                            .into_iter()
                            .map(|r| r[0] as usize)
                            .collect::<Vec<_>>()
                    } else {
                        (0..positions.len()).collect()
                    };
                    let normals = p["attributes"]["NORMAL"]
                        .as_u64()
                        .map(|i| accessor(root, bin, i as usize))
                        .transpose()?;
                    if let Some(normals) = &normals {
                        ensure!(
                            normals.len() == positions.len()
                                && normals
                                    .iter()
                                    .all(|n| n.len() == 3 && n.iter().all(|v| v.is_finite())),
                            "GLB NORMAL must contain one finite VEC3 per position"
                        );
                    }
                    let material = &root["materials"][p["material"].as_u64().unwrap_or(0) as usize]
                        ["pbrMetallicRoughness"]["baseColorFactor"];
                    let color = std::array::from_fn(|i| {
                        linear_to_srgb(material[i].as_f64().unwrap_or(0.6))
                    });
                    ensure!(
                        indices.len() % 3 == 0,
                        "GLB triangle index count is invalid"
                    );
                    for idx in indices.chunks(3) {
                        let mut v = [[0.; 3]; 3];
                        for j in 0..3 {
                            let p = positions
                                .get(idx[j])
                                .context("GLB triangle index outside POSITION")?;
                            ensure!(p.len() == 3, "GLB positions must be VEC3");
                            for k in 0..3 {
                                v[j][k] = (mat[k] * p[0]
                                    + mat[4 + k] * p[1]
                                    + mat[8 + k] * p[2]
                                    + mat[12 + k])
                                    * 1000.;
                            }
                            // glTF is right-handed Y-up; engineering meshes use Z-up.
                            v[j] = [v[j][0], -v[j][2], v[j][1]];
                        }
                        m.triangles.push(
                            Triangle {
                                vertices: v,
                                color,
                                normals: normals.as_ref().map(|normals| {
                                    let mut result = [[0.; 3]; 3];
                                    for j in 0..3 {
                                        if let Some(n) =
                                            normals.get(idx[j]).filter(|n| n.len() == 3)
                                        {
                                            result[j] = transform_normal(mat, [n[0], n[1], n[2]]);
                                        }
                                    }
                                    result
                                }),
                                object: n["name"].as_str().map(str::to_owned),
                                metallic:
                                    root["materials"][p["material"].as_u64().unwrap_or(0) as usize]
                                        ["pbrMetallicRoughness"]["metallicFactor"]
                                        .as_f64()
                                        .unwrap_or(1.)
                                        .clamp(0., 1.),
                                roughness:
                                    root["materials"][p["material"].as_u64().unwrap_or(0) as usize]
                                        ["pbrMetallicRoughness"]["roughnessFactor"]
                                        .as_f64()
                                        .unwrap_or(1.)
                                        .clamp(0.04, 1.),
                            },
                        );
                        ensure!(
                            m.triangles.len() <= 2_000_000,
                            "GLB exceeds two million triangles"
                        );
                    }
                }
            }
            for child in n["children"].as_array().into_iter().flatten() {
                walk(
                    root,
                    bin,
                    child.as_u64().context("Invalid GLB child")? as usize,
                    mat,
                    depth + 1,
                    m,
                )?;
            }
            Ok(())
        }
        let mut m = Self::default();
        let identity = [
            1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
        ];
        let scene = root["scene"].as_u64().unwrap_or(0) as usize;
        for n in root["scenes"][scene]["nodes"]
            .as_array()
            .context("GLB needs scene nodes")?
        {
            walk(
                &root,
                binary,
                n.as_u64().context("Invalid GLB scene node")? as usize,
                identity,
                0,
                &mut m,
            )?;
        }
        ensure!(!m.triangles.is_empty(), "GLB contains no triangle geometry");
        Ok(m)
    }
    pub fn export_stl(&self, path: &Path) -> Result<()> {
        let mut bytes = vec![0; 80];
        bytes[..20].copy_from_slice(b"Jenga triangle mesh ");
        bytes.extend((self.triangles.len() as u32).to_le_bytes());
        for t in &self.triangles {
            let a = sub(t.vertices[1], t.vertices[0]);
            let b = sub(t.vertices[2], t.vertices[0]);
            let normal = unit(cross(a, b));
            for v in std::iter::once(normal).chain(t.vertices) {
                for x in v {
                    bytes.extend((x as f32).to_le_bytes());
                }
            }
            bytes.extend([0, 0]);
        }
        crate::document::atomic_write(path, &bytes, false)
    }
    pub fn export_obj(&self, path: &Path) -> Result<()> {
        let mut text = String::from("# Jenga mesh; coordinates in millimetres\n");
        for t in &self.triangles {
            for v in t.vertices {
                text.push_str(&format!("v {} {} {}\n", v[0], v[1], v[2]));
            }
        }
        for i in 0..self.triangles.len() {
            text.push_str(&format!("f {} {} {}\n", i * 3 + 1, i * 3 + 2, i * 3 + 3));
        }
        crate::document::atomic_write(path, text.as_bytes(), false)
    }
}
pub fn sub(a: V3, b: V3) -> V3 {
    std::array::from_fn(|i| a[i] - b[i])
}
pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub fn unit(a: V3) -> V3 {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-15);
    a.map(|x| x / l)
}

/// Inverse-transpose transform preserves normals under non-uniform node scale.
fn transform_normal(m: [f64; 16], n: V3) -> V3 {
    let a = [m[0], m[1], m[2]];
    let b = [m[4], m[5], m[6]];
    let c = [m[8], m[9], m[10]];
    let x = cross(b, c);
    let y = cross(c, a);
    let z = cross(a, b);
    let determinant = a[0] * x[0] + a[1] * x[1] + a[2] * x[2];
    let sign = if determinant < 0. { -1. } else { 1. };
    let v: V3 = std::array::from_fn(|i| sign * (x[i] * n[0] + y[i] * n[1] + z[i] * n[2]));
    unit([v[0], -v[2], v[1]])
}

/// glTF baseColorFactor is linear; display textures are sRGB.
pub fn linear_to_srgb(c: f64) -> u8 {
    let c = c.clamp(0., 1.);
    ((if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1. / 2.4) - 0.055
    }) * 255.)
        .round() as u8
}
pub fn lit_color(color: [u8; 3], normal: [f64; 3], exposure: f64, ambient: f64) -> [u8; 3] {
    lit_material(color, normal, exposure, ambient, 0., 0.7)
}

/// Bounded studio shading with glTF metal/roughness and smooth vertex normals.
/// This is a preview shader, not a path-traced manufacturing/material simulation.
pub fn lit_material(
    color: [u8; 3],
    normal: V3,
    exposure: f64,
    ambient: f64,
    metallic: f64,
    roughness: f64,
) -> [u8; 3] {
    // Studio key, fill and rim lights. Opposing faces must not receive the
    // identical light produced by the previous absolute dot product.
    let n = unit(normal);
    let dot = |l: V3| (n[0] * l[0] + n[1] * l[1] + n[2] * l[2]).max(0.);
    let light = ambient.clamp(0., 1.) * 0.65
        + (1. - ambient.clamp(0., 1.) * 0.5)
            * (0.75 * dot(unit([0.3, -0.4, -0.85]))
                + 0.3 * dot(unit([-0.8, 0.2, -0.4]))
                + 0.2 * dot(unit([0.2, 0.9, 0.5])));
    color.map(|v| {
        let c = f64::from(v) / 255.;
        let linear = if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        };
        let half = unit([0.3, -0.4, -1.85]);
        let specular = dot(half).powf(2. / roughness.clamp(0.08, 1.).powi(2)) * 0.35;
        let reflectance = 0.04 * (1. - metallic) + linear * metallic;
        linear_to_srgb((linear * light + specular * reflectance) * exposure)
    })
}
#[cfg(test)]
mod lighting_tests {
    #[test]
    fn normals_use_inverse_transpose_and_engineering_up_axis() {
        let m = [
            2., 0., 0., 0., 0., 1., 0., 0., 0., 0., 0.5, 0., 0., 0., 0., 1.,
        ];
        let actual = super::transform_normal(m, super::unit([1., 1., 1.]));
        let expected = super::unit([0.5, -2., 1.]);
        for i in 0..3 {
            assert!((actual[i] - expected[i]).abs() < 1e-10);
        }
    }
    #[test]
    fn studio_lighting_distinguishes_opposing_faces() {
        assert_ne!(
            super::lit_color([80, 140, 80], [0., 0., 1.], 1., 0.5),
            super::lit_color([80, 140, 80], [0., 0., -1.], 1., 0.5)
        );
    }
    #[test]
    fn gltf_linear_materials_are_not_displayed_as_linear_bytes() {
        assert_eq!(super::linear_to_srgb(0.), 0);
        assert_eq!(super::linear_to_srgb(1.), 255);
        assert_eq!(super::linear_to_srgb(0.18), 118);
        assert!(
            super::lit_color([80, 140, 80], [0., 0., 1.], 2., 0.7)[1]
                > super::lit_color([80, 140, 80], [0., 0., 1.], 1., 0.7)[1]
        );
    }
}
