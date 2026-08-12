fn main() {
    println!("cargo:rerun-if-env-changed=TEMPLEFIX_UPDATER_PUBKEY");
    println!("cargo:rerun-if-env-changed=TEMPLEFIX_GITEE_UPDATE_ENDPOINT");
    tauri_build::build()
}
