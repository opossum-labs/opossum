use opossum_core::generic_validators::*;
use opm_macros_lib::EnsureValidated;

#[derive(EnsureValidated)]
struct User {
    name: Validated<String, AllNotEmpty>,

    #[validate(skip)]
    cache: Option<String>, // skipped, so allowed
}
fn main(){
}
