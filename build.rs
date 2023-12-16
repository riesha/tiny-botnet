fn main() {
    let dotenv_path = dotenv::dotenv().expect("failed to find .env file");
    println!("cargo:rerun-if-changed={}", dotenv_path.display());

    // Warning: `dotenv_iter()` is deprecated! Roll your own or use a maintained fork such as `dotenvy`.
    for env_var in dotenv::vars() {
        let (key, value) = env_var;
        println!("cargo:rustc-env={key}={value}");
    }
}
