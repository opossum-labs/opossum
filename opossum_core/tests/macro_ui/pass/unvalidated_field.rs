use opossum_core::generic_validators::*;
use opm_macros_lib::EnsureValidated;

#[derive(EnsureValidated)]
struct User {
    name: String, // ❌ not Validated or EnsureValidated type
}
fn main(){
}
