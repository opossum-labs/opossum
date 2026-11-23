use opossum_core::generic_validators::*;
use opm_macros_lib::EnsureValidated;

#[derive(EnsureValidated)]
struct Address {
    city: validated_type!(String, AllNotEmpty),
}

#[derive(EnsureValidated)]
struct User {
    address: Address,
    name: Validated<String, AllNotEmpty>,
}
fn main(){
}
