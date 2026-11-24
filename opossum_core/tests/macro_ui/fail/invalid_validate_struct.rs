use opm_macros_lib::EnsureValidated;
use opossum_core::generic_validators::ValidateTrait;

struct Validated<T, V>(T, V);
struct ValidatedVec<T, V>(Vec<T>, V);
struct AllNotEmpty;

#[derive(EnsureValidated)]
struct Address {
    street: Validated<String, AllNotEmpty>,
    city: Validated<String, AllNotEmpty>,
}

fn main(){
}