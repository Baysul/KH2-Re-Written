//! Sora posed from the Skate 3 rider. Each mapped Sora bone takes the
//! rider bone's rotation change from its rest pose, applied after a fixed
//! correction that turns Sora's bind bone onto the rider's rest bone (Sora
//! binds in a T-pose, the rider rests in an A-pose). Joint positions follow
//! Sora's own bone lengths; the pelvis sits on the rider's hips. Unmapped
//! bones (fingers, hair, clothes) keep their bind relation to their parent.
//!
//! Sora's bone choices are a test-room reading of P_EX100's bind skeleton
//! (T-pose, facing +Z, his left side on +X), not named data.
use bevy::math::{Mat4, Quat, Vec3};

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

/// One mapped bone: the Sora bone takes the rotation change of `rider`.
/// The rest correction turns Sora's bind direction `sora -> sora_tip` onto
/// the rider's rest direction `rider_from -> rider_to`. Ascending Sora order.
struct Limb {
    sora: usize,
    rider: usize,
    sora_from: usize,
    sora_tip: usize,
    rider_from: usize,
    rider_to: usize,
}

const fn limb(sora: usize, tip: usize, rider: usize, to: usize) -> Limb {
    Limb {
        sora,
        rider,
        sora_from: sora,
        sora_tip: tip,
        rider_from: rider,
        rider_to: to,
    }
}

/// A hand: its own rotation change, the forearm's rest correction.
const fn hand(sora: usize, rider: usize, forearm: &Limb) -> Limb {
    Limb {
        sora,
        rider,
        sora_from: forearm.sora_from,
        sora_tip: forearm.sora_tip,
        rider_from: forearm.rider_from,
        rider_to: forearm.rider_to,
    }
}

const LEFT_ELBOW: Limb = limb(79, 80, rider::LEFT_FOREARM, rider::LEFT_HAND);
const RIGHT_ELBOW: Limb = limb(171, 172, rider::RIGHT_FOREARM, rider::RIGHT_HAND);
const LIMBS: [Limb; 14] = {
    use rider::*;
    [
        limb(11, 12, LEFT_UP_LEG, LEFT_LEG),
        limb(12, 13, LEFT_LEG, LEFT_FOOT),
        limb(13, 17, LEFT_FOOT, LEFT_TOE),
        limb(40, 41, RIGHT_UP_LEG, RIGHT_LEG),
        limb(41, 42, RIGHT_LEG, RIGHT_FOOT),
        limb(42, 46, RIGHT_FOOT, RIGHT_TOE),
        limb(67, 68, SPINE1, NECK),
        limb(78, 79, LEFT_ARM, LEFT_FOREARM),
        LEFT_ELBOW,
        hand(80, LEFT_HAND, &LEFT_ELBOW),
        limb(140, 141, NECK, HEAD),
        limb(170, 171, RIGHT_ARM, RIGHT_FOREARM),
        RIGHT_ELBOW,
        hand(172, RIGHT_HAND, &RIGHT_ELBOW),
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
            .flat_map(|l| [l.sora, l.sora_tip])
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

    /// Sora's model-space bone matrices for the rider's bones now and at
    /// rest (rider model space, translations in Sora's units).
    pub fn pose(&self, now: &[Mat4], rest: &[Mat4]) -> Option<Vec<Mat4>> {
        if now.len() < rider::COUNT || rest.len() < rider::COUNT {
            return None;
        }
        let rotation = |m: &Mat4| m.to_scale_rotation_translation().1;
        let position = |m: &Mat4| m.w_axis.truncate();
        // The rotation change of a rider bone since its rest pose.
        let change = |b: usize| rotation(&now[b]) * rotation(&rest[b]).inverse();
        let pelvis = position(&self.bind[SORA_PELVIS]);
        let root = Mat4::from_translation(position(&now[rider::HIPS]))
            * Mat4::from_quat(change(rider::HIPS))
            * Mat4::from_translation(-pelvis);
        let mut world: Vec<Mat4> = Vec::with_capacity(self.bind.len());
        for (i, bind) in self.bind.iter().enumerate() {
            let follow = match self.parents[i] {
                p if p < 0 => root * *bind,
                p => world[p as usize] * self.bind_inverse[p as usize] * *bind,
            };
            let m = match LIMBS.iter().find(|l| l.sora == i) {
                Some(l) => {
                    let sora_dir = (position(&self.bind[l.sora_tip])
                        - position(&self.bind[l.sora_from]))
                    .normalize_or_zero();
                    let rider_dir = (position(&rest[l.rider_to]) - position(&rest[l.rider_from]))
                        .normalize_or_zero();
                    let correct = if sora_dir == Vec3::ZERO || rider_dir == Vec3::ZERO {
                        Quat::IDENTITY
                    } else {
                        Quat::from_rotation_arc(sora_dir, rider_dir)
                    };
                    let turned = (change(l.rider) * correct * rotation(bind)).normalize();
                    Mat4::from_rotation_translation(turned, position(&follow))
                }
                None => follow,
            };
            world.push(m);
        }
        Some(world)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 173-bone chain along +Y, every bone one unit above its parent.
    fn rig() -> Rig {
        let bind: Vec<Mat4> = (0..173)
            .map(|i| Mat4::from_translation(Vec3::new(0., i as f32, 0.)))
            .collect();
        let parents = (-1..172).collect();
        Rig::new(bind, parents).unwrap()
    }

    fn rider_rest() -> Vec<Mat4> {
        (0..rider::COUNT)
            .map(|i| Mat4::from_translation(Vec3::new(0., i as f32, 0.)))
            .collect()
    }

    #[test]
    fn mapped_bones_take_the_riders_rotation_change() {
        let r = rig();
        let rest = rider_rest();
        let mut now = rest.clone();
        // The rider's hips move; its left thigh turns 90 degrees about X.
        now[rider::HIPS] = Mat4::from_translation(Vec3::new(5., 90., 0.));
        let turn = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        now[rider::LEFT_UP_LEG] = Mat4::from_rotation_translation(turn, Vec3::new(0., 21., 0.));
        let pose = r.pose(&now, &rest).unwrap();
        // The pelvis sits on the rider's hips.
        let pelvis = pose[SORA_PELVIS].w_axis.truncate();
        assert!(
            (pelvis - Vec3::new(5., 90., 0.)).length() < 1e-3,
            "{pelvis}"
        );
        // Sora's thigh rotates by the same change (its rest correction is
        // identity: both chains point up).
        let (_, q, _) = pose[11].to_scale_rotation_translation();
        assert!(q.dot(turn).abs() > 1. - 1e-6, "{q} {turn}");
        // Its child keeps the bone length.
        let child = (pose[12].w_axis - pose[11].w_axis).truncate().length();
        assert!((child - 1.).abs() < 1e-4);
    }

    #[test]
    fn mismatched_skeletons_are_refused() {
        assert!(Rig::new(vec![Mat4::IDENTITY; 10], vec![-1; 10]).is_none());
    }
}
