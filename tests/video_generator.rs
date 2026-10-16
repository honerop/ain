use ain_compiler::compile;
use std::fs;
use std::process::Command;

#[test]
fn ain_video_renderer_generates_timed_svg_frames() {
    let compiled = compile(include_str!("../apps/video/render.ain"))
        .expect("the Ain video renderer must compile");
    assert!(compiled.rust.contains("ain_svgwrite("));

    let workspace = std::env::temp_dir().join(format!(
        "ain-video-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir_all(&workspace).unwrap();
    let rust_source = workspace.join("video.rs");
    let executable = workspace.join("video");
    let config = workspace.join("project.json");
    let frames = workspace.join("frames");

    fs::write(&rust_source, compiled.rust).unwrap();
    fs::write(
        &config,
        r##"{"width":200,"height":100,"fps":2,"scenes":[
          {"frames":3,"background":"#101825","elements":[
            {"kind":"circle","cx":30,"cy":45,"r":8,"fill":"#00ccff","from":{"cx":10}},
            {"kind":"text","x":50,"y":30,"value":"x < y","font_size":32,"anchor":"middle"}
          ]},
          {"frames":2,"background":"#123456","elements":[
            {"kind":"line","x1":5,"y1":5,"x2":100,"y2":50,"stroke":"white","stroke_width":4}
          ]}
        ]}"##,
    )
    .unwrap();

    let compile_output = Command::new("rustc")
        .arg("--edition=2021")
        .arg(&rust_source)
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("rustc must be available for native video test");
    assert!(
        compile_output.status.success(),
        "generated Rust failed: {}",
        String::from_utf8_lossy(&compile_output.stderr)
    );

    let run = Command::new(&executable)
        .env("AIN_VIDEO_SPEC", &config)
        .env("AIN_VIDEO_FRAMES", &frames)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "AIN renderer failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(stdout.contains("\"ok\":true"), "{stdout}");
    assert!(stdout.contains("\"frames\":5"), "{stdout}");

    let first = fs::read_to_string(frames.join("frame_000000.svg")).unwrap();
    let middle = fs::read_to_string(frames.join("frame_000001.svg")).unwrap();
    let last_first_scene = fs::read_to_string(frames.join("frame_000002.svg")).unwrap();
    let second_scene = fs::read_to_string(frames.join("frame_000003.svg")).unwrap();
    assert!(first.contains("cx=\"10\""), "{first}");
    assert!(middle.contains("cx=\"20\""), "{middle}");
    assert!(last_first_scene.contains("cx=\"30\""), "{last_first_scene}");
    assert!(first.contains("font-size=\"32\""), "{first}");
    assert!(first.contains("&lt;"), "{first}");
    assert!(second_scene.contains("<line "), "{second_scene}");
    assert!(second_scene.contains("stroke-width=\"4\""), "{second_scene}");
    assert!(second_scene.contains("fill=\"#123456\""), "{second_scene}");
    assert!(frames.join("frame_000004.svg").is_file());
    assert!(!frames.join("frame_000005.svg").exists());

    fs::remove_dir_all(workspace).unwrap();
}
