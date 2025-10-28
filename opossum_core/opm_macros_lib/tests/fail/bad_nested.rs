use opm_macros_lib::EnsureValidated;

#[derive(EnsureValidated)]
struct Inner {
    name: String, // ❌ invalid
}

#[derive(EnsureValidated)]
struct Outer {
    inner: Inner, // references invalid nested struct
}
fn main(){
}
