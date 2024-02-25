use crate::run_ttf_test;
use std::path::Path;

#[test]
fn noto_sans_regular() {
    run_ttf_test(Path::new("/Users/lstampfl/Programming/GitHub/subsetter/tests/fonts/NotoSans-Regular.ttf"), "*").unwrap();
    run_ttf_test(Path::new("/Users/lstampfl/Programming/GitHub/subsetter/tests/fonts/NotoSans-Regular.ttf"), "0").unwrap();
    run_ttf_test(Path::new("/Users/lstampfl/Programming/GitHub/subsetter/tests/fonts/NotoSans-Regular.ttf"), "1").unwrap();
    run_ttf_test(Path::new("/Users/lstampfl/Programming/GitHub/subsetter/tests/fonts/NotoSans-Regular.ttf"), "3").unwrap();
    run_ttf_test(Path::new("/Users/lstampfl/Programming/GitHub/subsetter/tests/fonts/NotoSans-Regular.ttf"), "3,6,8,9,11").unwrap();
    run_ttf_test(Path::new("/Users/lstampfl/Programming/GitHub/subsetter/tests/fonts/NotoSans-Regular.ttf"), "10-30").unwrap();
    run_ttf_test(Path::new("/Users/lstampfl/Programming/GitHub/subsetter/tests/fonts/NotoSans-Regular.ttf"), "30-50,132,137").unwrap();
}
