pub mod builds;

use std::path::PathBuf;
use crate::builds::*;


#[cfg(all(feature = "wdk-auto", feature = "bundled-wdk"))]
compile_error!(
    "Features `wdk-auto` and `bundled-wdk` are mutually exclusive. \
     Enable only one of them."
);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed={}", ENV_KM_LIB_OVERRIDE);
    println!("cargo:rerun-if-changed=builds.rs");

    #[cfg(feature = "bundled-wdk")]
    {
        let lib = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lib")
            .join(get_arch())
            .join("ntoskrnl.lib");

        println!("cargo:rustc-link-search=native={}", lib.parent().unwrap().display());
        println!("cargo:rustc-link-lib=static=ntoskrnl");

        Ok(())
    }

    #[cfg(feature = "wdk-auto")]
    {
        builds::auto::handle_auto_search()
    }

    #[cfg(not(any(feature = "bundled-wdk", feature = "wdk-auto")))]
    {
        Ok(())
    }
}