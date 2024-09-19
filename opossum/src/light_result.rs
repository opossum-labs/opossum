//! [`LightResult`] represents the [`LightData`], which arrives at a given (`OpticPort`)[`OpticPorts`] of an optical node.

use crate::{
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    rays::Rays,
};
use std::collections::HashMap;

pub type LightDings<T> = HashMap<String, T>;

pub type LightResult = LightDings<LightData>;

pub type LightRays = LightDings<Rays>;
pub type LightBouncingRays = LightDings<Vec<Rays>>;

pub fn light_result_to_light_rays(light_result: LightResult) -> OpmResult<LightRays> {
    let mut light_dings_rays = LightDings::<Rays>::new();
    for lr in light_result {
        let LightData::Geometric(r) = lr.1 else {
            return Err(OpossumError::Other(
                "no geometric rays data found in LightResult".into(),
            ));
        };
        light_dings_rays.insert(lr.0, r);
    }
    Ok(light_dings_rays)
}

pub fn light_rays_to_light_result(light_rays: LightRays) -> LightResult {
    let mut light_result = LightResult::default();
    for ld in light_rays {
        light_result.insert(ld.0, LightData::Geometric(ld.1));
    }
    light_result
}

pub fn light_rays_to_light_bouncing_rays(light_rays: LightRays) -> LightBouncingRays {
    let mut light_bouncing_rays = LightBouncingRays::default();
    for light_ray in light_rays {
        light_bouncing_rays.insert(light_ray.0, vec![light_ray.1]);
    }
    light_bouncing_rays
}

pub fn light_bouncing_rays_to_light_result(
    light_bouncing_rays: LightBouncingRays,
) -> OpmResult<LightResult> {
    let mut light_result = LightResult::default();
    for ld in light_bouncing_rays {
        let Some(rays) = ld.1.first() else {
            return Err(OpossumError::Other(
                "no rays found in LightBouncingRays".into(),
            ));
        };
        light_result.insert(ld.0, LightData::Geometric(rays.to_owned()));
    }
    Ok(light_result)
}
pub fn light_bouncing_rays_to_light_rays(
    light_bouncing_rays: LightBouncingRays,
    bounce: usize,
) -> OpmResult<LightRays> {
    let mut light_rays = LightRays::default();
    for lbr in light_bouncing_rays {
        let Some(rays) = lbr.1.get(bounce) else {
            return Err(OpossumError::Other(
                "bounce number not found in LightBouncingRays".into(),
            ));
        };
        light_rays.insert(lbr.0, rays.to_owned());
    }
    Ok(light_rays)
}
// pub fn merge_light_bouncing_rays_to_rays(light_bouncing_rays: LightBouncingRays) -> Rays {
//     let mut rays = Rays::default();
//     for lbr in light_bouncing_rays {
//         for bouncing_rays in lbr.1 {
//             rays.merge(&bouncing_rays);
//         }
//     }
//     rays
// }
