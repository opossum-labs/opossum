use opossum_core::generic_validators::*;
use opm_macros_lib::EnsureValidated;

#[derive(EnsureValidated)]
struct Address {
    street: Validated<String, AllNotEmpty>,
    city: Validated<String, AllNotEmpty>,
}
fn main(){
}
