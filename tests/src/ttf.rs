use crate::run_ttf_test;
use std::path::Path;

#[test]
fn noto_sans_regular() {
    run_ttf_test(Path::new("/Users/lstampfl/Programming/GitHub/subsetter/tests/fonts/NotoSans-Regular.ttf"), "*").unwrap();
    run_ttf_test(Path::new("/Users/lstampfl/Programming/GitHub/subsetter/tests/fonts/NotoSans-Regular.ttf"), "30-50,100-130,132,137").unwrap();
}
