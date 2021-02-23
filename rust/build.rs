//在添加一行测试是否删除ssh密钥
//添加一行代码试一试git更新
use std::env;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let hdr_out = Path::new(&out_dir).join("include/filcrypto.h");

    cbindgen::generate(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .expect("Could not generate header")
        .write_to_file(hdr_out);
}
