use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env::set_var("PROTOC", protobuf_src::protoc());

    println!("cargo:rerun-if-env-changed=BUILD_SHOW_GENPATH");
    if env::var("BUILD_SHOW_GENPATH").is_ok() {
        println!(
            "cargo:warning=Generating code into {}",
            env::var("OUT_DIR").unwrap()
        );
    }

    tonic_build::compile_protos("src/grpc_negotiator.proto")?;
    Ok(())
}
