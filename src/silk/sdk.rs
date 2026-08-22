#[allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals
)]
pub(crate) mod bindings {
    include!(concat!(env!("OUT_DIR"), "/silk_bindings.rs"));
}
