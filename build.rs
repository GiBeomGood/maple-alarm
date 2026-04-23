fn main() {
    println!("cargo:rerun-if-changed=assets/app.manifest");

    let mut resource = winresource::WindowsResource::new();
    resource.set_manifest_file("assets/app.manifest");
    resource.compile().expect("failed to embed application manifest");
}