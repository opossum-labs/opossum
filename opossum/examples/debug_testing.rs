use opossum::{
    joule, meter,
    surface::hit_map::{
        rays_hit_map::{EnergyHitPoint, FluenceHitPoint, HitPoint},
        HitMap,
    },
    J_per_cm2,
};
use uuid::Uuid;

fn main() {
    let uuid = Uuid::new_v4();
    let mut hm = HitMap::default();
    hm.add_to_hitmap(
        HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap()),
        1,
        &uuid,
    )
    .unwrap();
    assert!(hm
        .add_to_hitmap(
            HitPoint::Fluence(
                FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap()
            ),
            0,
            &uuid,
        )
        .is_err());
}
