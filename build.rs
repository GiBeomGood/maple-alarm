fn main() {
    println!("cargo:rerun-if-changed=assets/app.manifest");
    println!("cargo:rerun-if-changed=assets/icon.ico");

    let mut resource = winresource::WindowsResource::new();
    resource.set_manifest_file("assets/app.manifest");
    resource.set_icon("assets/icon.ico");
    resource.compile().expect("failed to embed application manifest");
}