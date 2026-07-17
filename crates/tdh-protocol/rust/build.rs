use std::fs;
use std::path::Path;

fn main() {
    // let protos_dir = Path::new("../protos");
    //
    // // Only regenerate when building from the source repo.
    // // Published crates ship the pre-generated .rs files and skip proto compilation.
    // if !protos_dir.exists() {
    //     return;
    // }
    //
    // let out_dir = std::path::PathBuf::from("./src");
    //
    // tonic_build::configure()
    //     .compile_well_known_types(true)
    //     .build_server(false)
    //     .build_client(false)
    //     .out_dir(&out_dir)
    //     .compile_protos(
    //         &[
    //             "../protos/common/consulting.proto",
    //             "../protos/common/patient.proto",
    //             "../protos/common/localization.proto",
    //             "../protos/common/events.proto",
    //             "../protos/onboarding/doctor.proto",
    //             "../protos/appointment/appointment.proto",
    //             "../protos/notification/notification.proto",
    //         ],
    //         &["../protos/"],
    //     )
    //     .expect("Failed to compile protos");
    //
    // let google_dir = out_dir.join("google");
    // let protobuf_dir = google_dir.join("protobuf");
    //
    // fs::create_dir_all(&protobuf_dir).expect("Failed to create google/protobuf directory");
    //
    // let google_protobuf_src = out_dir.join("google.protobuf.rs");
    // let google_protobuf_dst = protobuf_dir.join("mod.rs");
    // if google_protobuf_src.exists() {
    //     fs::rename(&google_protobuf_src, &google_protobuf_dst)
    //         .expect("Failed to move google.protobuf.rs");
    // }
    //
    // let mapping = [
    //     ("tdh.protocol.common.rs", "common.rs"),
    //     ("tdh.protocol.onboarding.rs", "onboarding.rs"),
    //     ("tdh.protocol.appointment.rs", "appointment.rs"),
    //     ("tdh.protocol.notification.rs", "notification.rs"),
    // ];
    //
    // for (old_name, new_name) in mapping {
    //     let old_path = out_dir.join(old_name);
    //     let new_path = out_dir.join(new_name);
    //     if old_path.exists() {
    //         fs::rename(&old_path, &new_path)
    //             .expect(&format!("Failed to rename {} to {}", old_name, new_name));
    //     }
    // }
    //
    // let common_rs = out_dir.join("common.rs");
    // if common_rs.exists() {
    //     let content = fs::read_to_string(&common_rs)
    //         .expect("Failed to read common.rs");
    //
    //     let fixed_content = content.replace(
    //         "super::super::super::super::google::protobuf::Empty",
    //         "crate::google::protobuf::Empty"
    //     );
    //
    //     fs::write(&common_rs, fixed_content)
    //         .expect("Failed to write fixed common.rs");
    // }
}
