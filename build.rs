use std::path::Path;

fn main() {
    #[cfg(target_os = "windows")]
    embed_resource::compile("assets/icon.rc", embed_resource::NONE);

    let _out_dir = std::env::var("OUT_DIR").unwrap();
    let shader_dir = Path::new("src/shaders");

    let compiler = shaderc::Compiler::new().expect("Failed to create shaderc compiler");

    for entry in std::fs::read_dir(shader_dir).expect("Failed to read shader dir") {
        let entry = entry.unwrap();
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let (stage, output_name) = match ext {
            "vert" => (
                shaderc::ShaderKind::Vertex,
                path.file_stem().unwrap().to_str().unwrap().to_string() + ".vert.spv",
            ),
            "frag" => (
                shaderc::ShaderKind::Fragment,
                path.file_stem().unwrap().to_str().unwrap().to_string() + ".frag.spv",
            ),
            _ => continue,
        };

        let source = std::fs::read_to_string(&path).expect("Failed to read shader");
        let name = path.file_name().unwrap().to_str().unwrap();

        let artifact = compiler
            .compile_into_spirv(&source, stage, name, "main", None)
            .expect("Failed to compile shader");

        let output_path = shader_dir.join(&output_name);
        std::fs::write(&output_path, artifact.as_binary_u8()).expect("Failed to write SPIR-V");

        println!("cargo:rerun-if-changed={}", path.display());
    }
}
