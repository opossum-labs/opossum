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
//pub type LightBouncingRays = LightDings<Vec<Rays>>;

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
