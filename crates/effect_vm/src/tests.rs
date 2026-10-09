use super::*;
use package_reader::effect::read_dpd;

fn put(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// One PDATA with one entry: a Move program (u0 = 0, u1 = 16) whose key 0
/// adds velocity (1, 2, 3); life 3 ticks, instance end 4 ticks.
fn synthetic() -> Arc<Dpd> {
    let mut b = vec![0u8; 0x300];
    put(&mut b, 0, 0x96);
    put(&mut b, 4, 1);
    put(&mut b, 8, 0x40);
    let block = 0x40 + 0x110;
    put(&mut b, block, 0x4000);
    put(&mut b, block + 0xc, 0x120);
    put(&mut b, block + 0x120, 1);
    put(&mut b, block + 0x124, 0x10);
    let e = block + 0x10;
    put(&mut b, e + 0x14, 0x3000);
    put(&mut b, e + 0x18, 0x7000_0000);
    put(&mut b, e + 0x1c, 0x7000_0000);
    put(&mut b, e + 0x20, 0xa0 + 32);
    b[e + 0x26] = 1;
    put(&mut b, e + 0x28, 36);
    b[e + 0x2c] = 16;
    b[e + 0x2e] = 1;
    put(&mut b, e + 0x30, 0x80);
    put(&mut b, e + 0x34, 0x94);
    put(&mut b, block + 0x84, 1f32.to_bits());
    put(&mut b, block + 0x88, 2f32.to_bits());
    put(&mut b, block + 0x8c, 3f32.to_bits());
    put(&mut b, block + 0x90, u32::MAX);
    put(&mut b, block + 0x94, 0);
    put(&mut b, block + 0x98, 16);
    Arc::new(read_dpd(&b).unwrap())
}

#[test]
fn ticks_at_half_rate_integrates_and_expires() {
    let mut inst = Instance::new(synthetic(), 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    let view = math::IDENTITY;
    let position = |inst: &Instance| {
        inst.parts
            .iter()
            .flatten()
            .next()
            .map(|p| [p.f32(0), p.f32(4), p.f32(8)])
    };
    // Frame 1 (odd half): the part spawns and runs its frame-0 update.
    inst.frame(&view, &mut rng);
    assert_eq!(position(&inst), Some([1., 2., 3.]));
    // Frame 2: updates skip (frame |= 1), then the tick advances.
    inst.frame(&view, &mut rng);
    assert_eq!(position(&inst), Some([1., 2., 3.]));
    inst.frame(&view, &mut rng);
    assert_eq!(position(&inst), Some([2., 4., 6.]));
    inst.frame(&view, &mut rng);
    inst.frame(&view, &mut rng);
    assert_eq!(position(&inst), Some([3., 6., 9.]));
    // After the third tick the part's frame reaches its life and it goes.
    inst.frame(&view, &mut rng);
    assert_eq!(inst.live_parts(), 0);
    assert!(!inst.finished());
    inst.frame(&view, &mut rng);
    inst.frame(&view, &mut rng);
    assert!(inst.finished());
}

#[test]
fn rng_matches_the_msvc_sequence() {
    let mut r = Rng::default();
    assert_eq!([r.rand(), r.rand(), r.rand()], [41, 18467, 6334]);
}
