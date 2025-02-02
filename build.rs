
use protobuf_codegen::Customize;

const PROTO_FILE: &str = "protobufs/diskuto.proto";

// Build will be re-run if any of these have changed:
const INPUTS: [&str; 1] = [
    PROTO_FILE
];

fn main() {
    for pattern in INPUTS {
        println!("cargo:rerun-if-changed={}", pattern);
    }
    protobuf_codegen::Codegen::new()
        .out_dir("src/protos")
        .inputs(&[PROTO_FILE])
        .include("protobufs")
        .customize(Customize::default()
            // We have our own protos.rs mod file, no need for a duplicate:
            .gen_mod_rs(false)
        )
        .run()
        .expect("protoc");

    // TODO: Do I need to place results here?
    // use std::env;
    // let out_dir = env::var("OUT_DIR").unwrap();
    // println!("cargo:warning=OUT_DIR={}", out_dir);
}