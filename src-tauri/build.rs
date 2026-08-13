fn main() {
    println!("cargo:rerun-if-changed=updater-public.key");
    println!("cargo:rerun-if-env-changed=TEMPLEFIX_GITEE_UPDATE_ENDPOINT");
    tauri_build::build()
}
