//! Translation of Processing-style GLSL fragment shaders into WGSL.
//!
//! The original sketch loads plain GLSL (`varying vec4 vertTexCoord`, `uniform sampler2D texture`,
//! `gl_FragColor`, loose `uniform float foo;` declarations, ...). wgpu needs Vulkan-style GLSL 450
//! (uniforms in blocks, separate texture/sampler objects, explicit locations), so this module
//! rewrites the source at load time and then lets `naga` turn it into WGSL. That keeps the
//! `data/Shaders/*.glsl` files shared between the Processing sketch and this port: new shaders can
//! be dropped in exactly as before.
//!
//! What the rewrite does:
//! * strips `#version`, `precision ...;` and Processing's `#define PROCESSING_*` markers
//! * gathers every non-sampler `uniform` into one std140 block (`bool` becomes `int` behind a macro)
//! * turns `uniform sampler2D <name>` into a texture + sampler pair and rewrites
//!   `texture2D(<name>, uv)` / `texture(<name>, uv)` (including `#define` aliases such as
//!   `#define iChannel0 texture`) into a lookup that flips v, so OpenGL's bottom-up texture
//!   coordinates still work against wgpu's top-down textures
//! * turns `varying` inputs into globals filled in by a generated `main()`; `vertTexCoord` comes
//!   from the vertex shader, `vertColor` is white (no tint)
//! * maps `gl_FragColor` to an output variable, and `gl_FragCoord` to a y-flipped copy so that the
//!   origin stays bottom-left as it is in Processing/OpenGL.

use regex::Regex;
use std::collections::HashMap;

/// Scalar/vector kinds a uniform can have.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UniformKind {
    Float,
    Int,
    Bool,
    Vec2,
    Vec3,
    Vec4,
    IVec2,
    Mat4,
}

#[derive(Clone, Debug)]
pub struct UniformField {
    pub kind: UniformKind,
    pub offset: u32,
}

/// Layout of the generated uniform block, keyed by the uniform name used in the GLSL source.
#[derive(Clone, Debug, Default)]
pub struct UniformLayout {
    pub fields: HashMap<String, UniformField>,
    pub size: u32,
}

#[derive(Debug)]
pub struct Translated {
    pub wgsl: String,
    pub layout: UniformLayout,
}

/// Output of [`rewrite`]: GLSL 450 source and the uniforms it declares.
pub struct Rewritten {
    pub glsl: String,
    pub uniforms: Vec<(String, UniformKind)>,
}

/// Name of the internal uniform that carries the render target size (used for gl_FragCoord flip).
pub const TARGET_SIZE_UNIFORM: &str = "_gsr_target_size";
/// Name of the dummy member that guarantees the block is never empty.
const PAD_UNIFORM: &str = "_gsr_pad";
const BOOL_PREFIX: &str = "_gsr_b_";

/// WGSL vertex shader shared by every fragment shader: a single full-screen triangle that hands
/// `vertTexCoord` (uv, 0, 1) to the fragment stage, with (0,0) at the bottom-left as in OpenGL.
pub const FULLSCREEN_VS_WGSL: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec4<f32>,
};

@vertex
fn main(@builtin(vertex_index) idx: u32) -> VsOut {
    var out: VsOut;
    let x = f32(i32(idx & 1u) * 4 - 1);
    let y = f32(i32(idx >> 1u) * 4 - 1);
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    // OpenGL convention: (0,0) is the bottom-left corner.
    out.uv = vec4<f32>((x + 1.0) * 0.5, (y + 1.0) * 0.5, 0.0, 1.0);
    return out;
}
"#;

fn kind_from_glsl(ty: &str) -> Option<(UniformKind, &'static str)> {
    // returns the kind and the GLSL type actually placed in the block
    Some(match ty {
        "float" => (UniformKind::Float, "float"),
        "int" => (UniformKind::Int, "int"),
        "bool" => (UniformKind::Bool, "int"),
        "vec2" => (UniformKind::Vec2, "vec2"),
        "vec3" => (UniformKind::Vec3, "vec3"),
        "vec4" => (UniformKind::Vec4, "vec4"),
        "ivec2" => (UniformKind::IVec2, "ivec2"),
        "mat4" => (UniformKind::Mat4, "mat4"),
        _ => return None,
    })
}

/// Rewrite Processing GLSL into Vulkan GLSL 450 (exposed for tests/debugging).
pub fn rewrite(source: &str) -> Result<Rewritten, String> {
    let mut src = source.replace("\r\n", "\n");

    // Drop things that would upset the GLSL 450 frontend.
    let version = Regex::new(r"(?m)^\s*#version[^\n]*$").unwrap();
    src = version.replace_all(&src, "").into_owned();
    let precision = Regex::new(r"(?m)^\s*precision\s+\w+\s+\w+\s*;").unwrap();
    src = precision.replace_all(&src, "").into_owned();
    let proc_define = Regex::new(r"(?m)^\s*#define\s+PROCESSING_\w+[^\n]*$").unwrap();
    src = proc_define.replace_all(&src, "").into_owned();

    // Samplers.
    let sampler_re = Regex::new(r"(?m)^\s*uniform\s+sampler2D\s+(\w+)\s*;[^\n]*$").unwrap();
    let mut sampler_names: Vec<String> = sampler_re
        .captures_iter(&src)
        .map(|c| c[1].to_string())
        .collect();
    src = sampler_re.replace_all(&src, "").into_owned();
    // `#define iChannel0 texture` style aliases for a sampler.
    let alias_re = Regex::new(r"(?m)^\s*#define\s+(\w+)\s+(\w+)\s*$").unwrap();
    let mut aliases = Vec::new();
    for c in alias_re.captures_iter(&src) {
        if sampler_names.iter().any(|s| s == &c[2]) {
            aliases.push(c[1].to_string());
        }
    }
    for a in &aliases {
        let line = Regex::new(&format!(
            r"(?m)^\s*#define\s+{}\s+\w+\s*$",
            regex::escape(a)
        ))
        .unwrap();
        src = line.replace_all(&src, "").into_owned();
    }
    sampler_names.extend(aliases);
    let uses_texture = !sampler_names.is_empty();
    for name in &sampler_names {
        let call = Regex::new(&format!(
            r"\b(?:texture2D|texture)\s*\(\s*{}\s*,",
            regex::escape(name)
        ))
        .unwrap();
        src = call.replace_all(&src, "_gsr_texture(").into_owned();
    }

    // Plain uniforms -> block members.
    let uniform_re = Regex::new(r"(?m)^\s*uniform\s+(\w+)\s+([\w\s,]+?)\s*;[^\n]*$").unwrap();
    let mut uniforms: Vec<(String, UniformKind)> = Vec::new();
    let mut members = String::new();
    let mut bool_defines = String::new();
    for c in uniform_re.captures_iter(&src) {
        let ty = &c[1];
        let Some((kind, block_ty)) = kind_from_glsl(ty) else {
            return Err(format!("unsupported uniform type '{ty}'"));
        };
        for name in c[2].split(',').map(str::trim).filter(|n| !n.is_empty()) {
            if uniforms.iter().any(|(n, _)| n == name) {
                continue;
            }
            uniforms.push((name.to_string(), kind));
            if kind == UniformKind::Bool {
                members.push_str(&format!("    {block_ty} {BOOL_PREFIX}{name};\n"));
                bool_defines.push_str(&format!("#define {name} ({BOOL_PREFIX}{name} != 0)\n"));
            } else {
                members.push_str(&format!("    {block_ty} {name};\n"));
            }
        }
    }
    src = uniform_re.replace_all(&src, "").into_owned();
    members.push_str(&format!(
        "    vec2 {TARGET_SIZE_UNIFORM};\n    float {PAD_UNIFORM};\n"
    ));

    // Varyings -> globals initialised in the generated main().
    let varying_re = Regex::new(r"(?m)^\s*(?:varying|in)\s+(\w+)\s+(\w+)\s*;[^\n]*$").unwrap();
    let mut varying_init = String::new();
    let mut varying_decls = String::new();
    for c in varying_re.captures_iter(&src) {
        let ty = c[1].to_string();
        let name = c[2].to_string();
        varying_decls.push_str(&format!("{ty} {name};\n"));
        let value = match (name.as_str(), ty.as_str()) {
            ("vertTexCoord", "vec4") => "_gsr_vtc".to_string(),
            ("vertTexCoord", "vec2") => "_gsr_vtc.xy".to_string(),
            ("vertTexCoord", "vec3") => "_gsr_vtc.xyz".to_string(),
            ("vertColor", "vec4") => "vec4(1.0)".to_string(),
            (_, "vec4") => "_gsr_fc".to_string(),
            (_, "vec3") => "_gsr_fc.xyz".to_string(),
            (_, "vec2") => "_gsr_fc.xy".to_string(),
            (_, "float") => "0.0".to_string(),
            _ => return Err(format!("unsupported varying '{ty} {name}'")),
        };
        varying_init.push_str(&format!("    {name} = {value};\n"));
    }
    src = varying_re.replace_all(&src, "").into_owned();

    // Builtins.
    let frag_color = Regex::new(r"\bgl_FragColor\b").unwrap();
    src = frag_color.replace_all(&src, "_gsr_out").into_owned();
    let frag_coord = Regex::new(r"\bgl_FragCoord\b").unwrap();
    src = frag_coord.replace_all(&src, "_gsr_fc").into_owned();
    let main_re = Regex::new(r"\bvoid\s+main\s*\(\s*(?:void)?\s*\)").unwrap();
    if !main_re.is_match(&src) {
        return Err("no main() found".into());
    }
    src = main_re.replace(&src, "void _gsr_user_main()").into_owned();
    // naga rejects calls to functions that are only defined further down (Shadertoy wrappers do
    // `void main() { mainImage(...); }` above `mainImage`), so move the user main to the end.
    src = hoist_function_to_end(&src, "void _gsr_user_main()");
    // Function prototypes (`void mainImage(out vec4 c, in vec2 p);`) also trip naga up; with main
    // moved to the end they are no longer needed.
    let proto_re = Regex::new(r"(?m)^\s*(\w+)\s+\w+\s*\([^;{}()]*\)\s*;[ \t]*$").unwrap();
    src = proto_re
        .replace_all(&src, |c: &regex::Captures| {
            if matches!(&c[1], "return" | "else") {
                c[0].to_string()
            } else {
                String::new()
            }
        })
        .into_owned();

    let mut out = String::new();
    out.push_str("#version 450\n");
    out.push_str("layout(set = 0, binding = 0) uniform GsrParams {\n");
    out.push_str(&members);
    out.push_str("};\n");
    out.push_str(&bool_defines);
    if uses_texture {
        out.push_str("layout(set = 0, binding = 1) uniform texture2D _gsr_tex;\n");
        out.push_str("layout(set = 0, binding = 2) uniform sampler _gsr_samp;\n");
        // Textures are stored top-down in wgpu but bottom-up in OpenGL; flip lookups so that
        // coordinates computed from gl_FragCoord or vertTexCoord land where they did in GL.
        out.push_str(
            "vec4 _gsr_texture(vec2 uv) { return texture(sampler2D(_gsr_tex, _gsr_samp), vec2(uv.x, 1.0 - uv.y)); }\n",
        );
    }
    out.push_str("layout(location = 0) in vec4 _gsr_vtc;\n");
    out.push_str("layout(location = 0) out vec4 _gsr_out;\n");
    out.push_str("vec4 _gsr_fc;\n");
    out.push_str(&varying_decls);
    out.push_str("#line 1\n");
    out.push_str(&src);
    out.push_str("\nvoid main() {\n");
    out.push_str(&format!(
        "    _gsr_fc = vec4(gl_FragCoord.x, {TARGET_SIZE_UNIFORM}.y - gl_FragCoord.y, gl_FragCoord.z, gl_FragCoord.w);\n"
    ));
    out.push_str(&varying_init);
    out.push_str("    _gsr_out = vec4(0.0);\n");
    out.push_str("    _gsr_user_main();\n}\n");

    uniforms.push((TARGET_SIZE_UNIFORM.to_string(), UniformKind::Vec2));
    Ok(Rewritten {
        glsl: out,
        uniforms,
    })
}

/// Cut the function starting with `signature` (through its matching closing brace) and append it.
fn hoist_function_to_end(src: &str, signature: &str) -> String {
    let Some(start) = src.find(signature) else {
        return src.to_string();
    };
    let Some(open_rel) = src[start..].find('{') else {
        return src.to_string();
    };
    let mut depth = 0usize;
    let mut end = None;
    for (i, ch) in src[start + open_rel..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + open_rel + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(end) = end else {
        return src.to_string();
    };
    let func = &src[start..end];
    format!("{}{}\n{}\n", &src[..start], &src[end..], func)
}

fn format_parse_errors(errors: &[naga::front::glsl::Error], src: &str) -> String {
    let mut s = String::new();
    for e in errors {
        let loc = e.meta.location(src);
        s.push_str(&format!("  line {}: {}\n", loc.line_number, e.kind));
    }
    s
}

/// Translate a Processing GLSL fragment shader into WGSL plus a uniform layout description.
pub fn translate(source: &str) -> Result<Translated, String> {
    let Rewritten { glsl, uniforms } = rewrite(source)?;

    let mut parser = naga::front::glsl::Frontend::default();
    let options = naga::front::glsl::Options::from(naga::ShaderStage::Fragment);
    let module = parser
        .parse(&options, &glsl)
        .map_err(|errs| format!("GLSL parse failed:\n{}", format_parse_errors(&errs, &glsl)))?;

    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|e| format!("validation failed: {e:?}"))?;

    // Pull std140 offsets back out of the module so the CPU side writes exactly what naga laid out.
    let mut layout = UniformLayout::default();
    for (_, var) in module.global_variables.iter() {
        if var.space != naga::AddressSpace::Uniform {
            continue;
        }
        if let naga::TypeInner::Struct { members, span } = &module.types[var.ty].inner {
            layout.size = (*span).max(16);
            for m in members {
                let Some(mname) = &m.name else { continue };
                let src_name = mname.strip_prefix(BOOL_PREFIX).unwrap_or(mname);
                if let Some((_, kind)) = uniforms.iter().find(|(n, _)| n == src_name) {
                    layout.fields.insert(
                        src_name.to_string(),
                        UniformField {
                            kind: *kind,
                            offset: m.offset,
                        },
                    );
                }
            }
        }
    }

    let wgsl =
        naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
            .map_err(|e| format!("WGSL output failed: {e}"))?;

    Ok(Translated { wgsl, layout })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn shader_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/Shaders")
    }

    /// Every shader the sketch actually uses must translate.
    #[test]
    fn used_shaders_translate() {
        let used = [
            "blurFrag.glsl",
            "brcosa.glsl",
            "hue.glsl",
            "pixelate.glsl",
            "channels.glsl",
            "threshold.glsl",
            "neon.glsl",
            "deform.glsl",
            "pixelrolls.glsl",
            "modcolor.glsl",
            "halftone.glsl",
            "invert.glsl",
            "strobe.glsl",
            "neonSpark.glsl",
            "voronoiSlice.glsl",
            "vhs_glitch.glsl",
            "sobel.glsl",
            "kaleidoscope.glsl",
            "visCandywarp.glsl",
            "visJazzUniverse.glsl",
            "visCircleTunnel.glsl",
            "visSpirographSpectrum.glsl",
            "vis1 - waves.glsl",
            "visBubbles.glsl",
            "visIsoHatching.glsl",
            "visOpArt.glsl",
            "visOrbs.glsl",
        ];
        let mut failures = Vec::new();
        for f in used {
            let src = std::fs::read_to_string(shader_dir().join(f)).unwrap();
            if let Err(e) = translate(&src) {
                failures.push(format!("{f}: {e}"));
            }
        }
        assert!(failures.is_empty(), "\n{}", failures.join("\n"));
    }

    #[test]
    fn uniform_offsets_are_std140() {
        let src = "uniform float a;\nuniform vec3 b;\nuniform bool c;\nvoid main(){ gl_FragColor = vec4(a, b.x, c ? 1.0 : 0.0, 1.0); }";
        let t = translate(src).unwrap();
        assert_eq!(t.layout.fields["a"].offset, 0);
        assert_eq!(t.layout.fields["b"].offset, 16);
        assert_eq!(t.layout.fields["c"].offset, 28);
        assert!(t.layout.size.is_multiple_of(16));
    }
}
