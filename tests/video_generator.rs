use ain_compiler::compile;
use std::fs;
use std::process::Command;

#[test]
fn ain_only_renderer_writes_valid_y4m_frames() {
    // The video application must remain ordinary Ain, with no new compiler builtins.
    let compiled = compile(include_str!("../apps/video/render.ain"))
        .expect("Ain-only video source must compile");
    assert!(compiled.rust.contains("ain_fs_write("));
    assert!(!compiled.rust.contains("ain_process_run("));

    let workspace = std::env::temp_dir().join(format!(
        "ain-y4m-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir_all(&workspace).unwrap();
    let generated = workspace.join("video.rs");
    let binary = workspace.join("video");
    let json = workspace.join("movie.json");
    let output = workspace.join("movie.y4m");

    fs::write(&generated, compiled.rust).unwrap();
    fs::write(
        &json,
        r#"{"width":16,"height":16,"fps":2,"scenes":[
          {"frames":2,"background":"black","elements":[
            {"kind":"rect","x":0,"y":0,"width":8,"height":8,
             "fill":"white","from":{"x":0},"to":{"x":4}}
          ]},
          {"frames":1,"background":"navy","elements":[]}
        ]}"#,
    )
    .unwrap();

    let built = Command::new("rustc")
        .args(["--edition=2021"])
        .arg(&generated)
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("rustc must be installed for the native Ain backend");
    assert!(
        built.status.success(),
        "Ain generated Rust did not compile: {}",
        String::from_utf8_lossy(&built.stderr)
    );

    let run = Command::new(&binary)
        .env("AIN_VIDEO_SPEC", &json)
        .env("AIN_VIDEO_OUTPUT", &output)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "Ain video generator failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(String::from_utf8_lossy(&run.stdout).contains("Wrote 3 Y4M frames"));

    let actual = fs::read(&output).unwrap();
    let header = b"YUV4MPEG2 W16 H16 F2:1 Ip A1:1 C420jpeg\n";
    let plane_length = 16 * 16;
    let chroma_length = plane_length / 4;
    let frame_length = 6 + plane_length + 2 * chroma_length;
    assert!(actual.starts_with(header));
    assert_eq!(actual.len(), header.len() + frame_length * 3);

    let first = header.len();
    assert_eq!(&actual[first..first + 6], b"FRAME\n");
    assert_eq!(actual[first + 6], b'~'); // Rectangle at (0,0), frame 0
    assert_eq!(actual[first + 6 + 12], b' '); // Background at (12,0)

    let second = first + frame_length;
    assert_eq!(&actual[second..second + 6], b"FRAME\n");
    assert_eq!(actual[second + 6], b' '); // Rectangle moved right
    assert_eq!(actual[second + 6 + 4], b'~');

    let third = second + frame_length;
    assert_eq!(&actual[third..third + 6], b"FRAME\n");
    assert_eq!(actual[third + 6], b'0'); // Second scene background
    assert_eq!(actual[third + 6 + plane_length], b'~'); // U plane
    assert_eq!(actual[third + 6 + plane_length + chroma_length], b'~'); // V plane

    // Also exercise the documented example (animated text, rectangle, circle).
    fs::write(
        &json,
        include_str!("../apps/video/examples/explainer.json"),
    ).unwrap();
    let demo_path = workspace.join("explainer.y4m");
    let demo_run = Command::new(&binary)
        .env("AIN_VIDEO_SPEC", &json)
        .env("AIN_VIDEO_OUTPUT", &demo_path)
        .output()
        .unwrap();
    assert!(
        demo_run.status.success(),
        "Example render failed: {}",
        String::from_utf8_lossy(&demo_run.stderr)
    );
    let demo = fs::read(&demo_path).unwrap();
    let demo_header = b"YUV4MPEG2 W96 H64 F8:1 Ip A1:1 C420jpeg\n";
    let demo_frame_length = 6 + 96 * 64 * 3 / 2;
    assert!(demo.starts_with(demo_header));
    assert_eq!(demo.len(), demo_header.len() + 16 * demo_frame_length);

    fs::remove_dir_all(&workspace).unwrap();
}
