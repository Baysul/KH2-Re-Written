//! Skate 3 TU3 82DA8BE0, 82DAC880 and 82DAC9D0.
//! Track the skater root while a trick animates; otherwise track the board
//! projected against reckoning up. Deck flicks are not skater rotations.
pub(super) type Basis = [[f32; 3]; 3];
const IDENTITY: Basis = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalized(v: [f32; 3]) -> Option<[f32; 3]> {
    let length = dot(v, v).sqrt();
    (length > 1e-6 && length.is_finite()).then(|| v.map(|x| x / length))
}
#[derive(Clone, Copy)]
struct Tracker {
    previous: Basis,
    angle: f32,
}
impl Tracker {
    fn reset(&mut self, basis: Basis) {
        self.previous = basis;
        self.angle = 0.;
    }
    fn update(&mut self, basis: Basis) {
        let up = self.previous[1];
        // Native double cross projects the new Z onto the previous Y plane.
        if let Some(forward) = normalized(cross(cross(up, basis[2]), up)) {
            let cosine = dot(forward, self.previous[2]);
            // 82DACB20 rejects a discontinuity beyond a quarter turn per tick.
            if cosine >= 0. {
                self.angle += dot(cross(self.previous[2], forward), up).atan2(cosine);
            }
        }
        self.previous = basis;
    }
}
pub(super) struct Rotation {
    player: Tracker,
    board: Tracker,
    banked: f32,
    both: bool,
}
impl Default for Rotation {
    fn default() -> Self {
        Self {
            player: Tracker {
                previous: IDENTITY,
                angle: 0.,
            },
            board: Tracker {
                previous: IDENTITY,
                angle: 0.,
            },
            banked: 0.,
            both: false,
        }
    }
}
impl Rotation {
    pub fn reset(&mut self, player: Basis, board: Basis) {
        self.player.reset(player);
        self.board.reset(board);
        self.banked = 0.;
        self.both = false;
    }
    pub fn update(&mut self, player: Basis, board: Basis, up: [f32; 3], trick: bool) -> f32 {
        self.player.update(player);
        if trick {
            if self.both {
                self.banked += self.player.angle;
                self.player.reset(player);
                self.both = false;
            }
            self.board.reset(board);
        } else if self.both {
            // 82D2CDC0 builds an orthonormal frame from reckoning up/deck Z.
            if let Some(right) = normalized(cross(up, board[2])) {
                if let Some(forward) = normalized(cross(right, up)) {
                    self.board.update([right, up, forward]);
                }
            }
        } else {
            self.banked += self.player.angle;
            self.player.reset(player);
            self.board.reset(board);
            self.both = true;
        }
        self.banked + self.board.angle + if self.both { 0. } else { self.player.angle }
    }
}
