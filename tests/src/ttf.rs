use crate::run_ttf_test;
use std::path::Path;

#[test]
fn noto_sans_regular() {
    run_ttf_test("NotoSans-Regular.ttf", "*").unwrap();
    run_ttf_test("NotoSans-Regular.ttf", "0").unwrap();
    run_ttf_test("NotoSans-Regular.ttf", "1").unwrap();
    run_ttf_test("NotoSans-Regular.ttf", "3").unwrap();
    run_ttf_test("NotoSans-Regular.ttf", "3,6,8,9,11").unwrap();
    run_ttf_test("NotoSans-Regular.ttf", "10-30").unwrap();
    run_ttf_test("NotoSans-Regular.ttf", "30-50,132,137").unwrap();
    run_ttf_test(
        "NotoSans-Regular.ttf",
        "20-25,30,40,45,47,48,52-70,300-350,500-522,3001",
    )
    .unwrap();
}
