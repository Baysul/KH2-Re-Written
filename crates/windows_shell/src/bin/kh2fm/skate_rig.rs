//! Sora posed from the Skate 3 rider: a direction retarget onto Sora's
//! bind pose. The hips frame and position come from the rider's hips; each
//! mapped limb bone swings so it points where the rider's bone points.
//! Twist and unmapped bones (hands, hair, clothes) keep their bind
//! relation to their parent.
//!
//! Sora's bone choices are a test-room reading of P_EX100's bind skeleton
//! (T-pose, facing +Z, his left side on +X), not named data.
use bevy::math::{Mat3, Mat4, Quat, Vec3};

/// Rider bones (skate-sim's skeleton order).
mod rider {
    pub const HIPS: usize = 1;
    pub const SPINE1: usize = 3;
    pub const NECK: usize = 6;
    pub const HEAD: usize = 8;
    pub const RIGHT_ARM: usize = 10;
    pub const RIGHT_FOREARM: usize = 11;
    pub const RIGHT_HAND: usize = 12;
    pub const LEFT_ARM: usize = 14;
    pub const LEFT_FOREARM: usize = 15;
    pub const LEFT_HAND: usize = 16;
    pub const RIGHT_UP_LEG: usize = 17;
    pub const RIGHT_LEG: usize = 18;
    pub const RIGHT_FOOT: usize = 19;
    pub const RIGHT_TOE: usize = 20;
    pub const LEFT_UP_LEG: usize = 21;
    pub const LEFT_LEG: usize = 22;
    pub const LEFT_FOOT: usize = 23;
    pub const LEFT_TOE: usize = 24;
    pub const COUNT: usize = 25;
}

/// Sora's pelvis (the bone the rider's hips place).
const SORA_PELVIS: usize = 3;

/// (Sora bone, Sora tip bone, rider from, rider to): the Sora bone turns so
/// bone -> tip points along rider from -> to. Ascending Sora order.
const LIMBS: [(usize, usize, usize, usize); 12] = {
    use rider::*;
    [
        (11, 12, LEFT_UP_LEG, LEFT_LEG),
        (12, 13, LEFT_LEG, LEFT_FOOT),
        (13, 17, LEFT_FOOT, LEFT_TOE),
        (40, 41, RIGHT_UP_LEG, RIGHT_LEG),
        (41, 42, RIGHT_LEG, RIGHT_FOOT),
        (42, 46, RIGHT_FOOT, RIGHT_TOE),
        (67, 68, SPINE1, NECK),
        (78, 79, LEFT_ARM, LEFT_FOREARM),
        (79, 80, LEFT_FOREARM, LEFT_HAND),
        (140, 141, NECK, HEAD),
        (170, 171, RIGHT_ARM, RIGHT_FOREARM),
        (171, 172, RIGHT_FOREARM, RIGHT_HAND),
    ]
};

pub struct Rig {
    bind: Vec<Mat4>,
    bind_inverse: Vec<Mat4>,
    parents: Vec<i32>,
}

impl Rig {
    /// `bind`: Sora's bind matrices (model space); `parents` per bone.
    /// None when the skeleton is not the one the bone choices were read from.
    pub fn new(bind: Vec<Mat4>, parents: Vec<i32>) -> Option<Self> {
        let used = LIMBS
            .iter()
            .flat_map(|l| [l.0, l.1])
            .chain([SORA_PELVIS])
            .max()?;
        if bind.len() != parents.len()
            || used >= bind.len()
            || parents.iter().enumerate().any(|(i, &p)| p >= i as i32)
        {
            return None;
        }
        let bind_inverse = bind.iter().map(Mat4::inverse).collect();
        Some(Self {
            bind,
            bind_inverse,
            parents,
        })
    }

    /// Sora's model-space bone matrices for the rider's bone positions
    /// (rider model space, scaled to Sora's units).
    pub fn pose(&self, rider: &[Vec3]) -> Option<Vec<Mat4>> {
        if rider.len() < rider::COUNT {
            return None;
        }
        // The rider's hips frame: left across the up-legs, up the spine.
        let left = (rider[rider::LEFT_UP_LEG] - rider[rider::RIGHT_UP_LEG]).normalize_or_zero();
        let up = (rider[rider::SPINE1] - rider[rider::HIPS]).normalize_or_zero();
        let forward = left.cross(up).normalize_or_zero();
        if forward == Vec3::ZERO {
            return None;
        }
        let up = forward.cross(left);
        let turn = Quat::from_mat3(&Mat3::from_cols(left, up, forward));
        let pelvis = self.bind[SORA_PELVIS].w_axis.truncate();
        let root = Mat4::from_translation(rider[rider::HIPS])
            * Mat4::from_quat(turn)
            * Mat4::from_translation(-pelvis);
        let mut world = Vec::with_capacity(self.bind.len());
        for (i, bind) in self.bind.iter().enumerate() {
            let mut m = match self.parents[i] {
                p if p < 0 => root * *bind,
                p => world[p as usize] * self.bind_inverse[p as usize] * *bind,
            };
            if let Some(&(_, tip, from, to)) = LIMBS.iter().find(|l| l.0 == i) {
                let at = m.w_axis.truncate();
                let tip = (m * self.bind_inverse[i] * self.bind[tip])
                    .w_axis
                    .truncate();
                let current = (tip - at).normalize_or_zero();
                let wanted = (rider[to] - rider[from]).normalize_or_zero();
                if current != Vec3::ZERO && wanted != Vec3::ZERO {
                    m = Mat4::from_translation(at)
                        * Mat4::from_quat(Quat::from_rotation_arc(current, wanted))
                        * Mat4::from_translation(-at)
                        * m;
                }
            }
            world.push(m);
        }
        Some(world)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 173-bone chain skeleton laid out like the bind pose along +Y.
    fn rig() -> Rig {
        let bind: Vec<Mat4> = (0..173)
            .map(|i| Mat4::from_translation(Vec3::new(0., i as f32, 0.)))
            .collect();
        let parents = (-1..172).collect();
        Rig::new(bind, parents).unwrap()
    }

    #[test]
    fn mapped_bones_follow_rider_directions() {
        let r = rig();
        let mut rider = vec![Vec3::ZERO; rider::COUNT];
        rider[rider::HIPS] = Vec3::new(0., 90., 0.);
        rider[rider::SPINE1] = Vec3::new(0., 110., 0.);
        rider[rider::LEFT_UP_LEG] = Vec3::new(8., 90., 0.);
        rider[rider::RIGHT_UP_LEG] = Vec3::new(-8., 90., 0.);
        // The left thigh points straight forward.
        rider[rider::LEFT_LEG] = Vec3::new(8., 90., 40.);
        let pose = r.pose(&rider).unwrap();
        // The pelvis sits on the rider's hips.
        assert!((pose[SORA_PELVIS].w_axis.truncate() - rider[rider::HIPS]).length() < 1e-3);
        let thigh = (pose[12].w_axis - pose[11].w_axis).truncate().normalize();
        assert!((thigh - Vec3::Z).length() < 1e-4, "{thigh}");
        // Children keep their offsets from their parent.
        let child = (pose[13].w_axis - pose[12].w_axis).truncate().length();
        assert!((child - 1.).abs() < 1e-4);
    }

    #[test]
    fn mismatched_skeletons_are_refused() {
        assert!(Rig::new(vec![Mat4::IDENTITY; 10], vec![-1; 10]).is_none());
    }
}
