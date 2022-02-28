// Uncomment this file when we want to generate new protos.
// This is only here until we start publishing our protos.
// This needs to stay commented out because when we `brew install`
// the cli, rust is unable to build the protos. So we are building
// them, and then commenting out the build file until we need to
// rebuild them.

// use std::io::Result;

// fn main() -> Result<()> {
//     tonic_build::configure()
//         .build_server(false)
//         .out_dir("src/generated")
//         .compile(
//             &[
//                 "src/protos/controlclient.proto",
//                 "src/protos/cacheclient.proto",
//             ],
//             &["src/protos"],
//         )
//         .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
//     Ok(())
// }

fn main() {
    println!("Placeholder build function");
}
