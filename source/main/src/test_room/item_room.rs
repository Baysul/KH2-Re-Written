//! Sora's battle items in the test room: the item slots, the Items
//! submenu rows, the use command (motion 9, `Player::use_item`) and the
//! heal that lands 30 ticks after the use trigger (FUN_1403f6790,
//! FUN_1403f6960, FUN_1403f6d00, FUN_1403c4b80).
use super::*;
use std::collections::BTreeMap;

/// The wait between the use trigger and the heal (`DAT_140623bcc`).
const HEAL_DELAY: f32 = 30.;

/// One consumable's 03system record fields.
#[derive(Clone, Copy, Debug)]
pub struct ItemInfo {
    /// +4: the amount, in percent of max HP / MP.
    pub amount: u16,
    /// +0x10: the command that uses it.
    pub command: u16,
}

pub struct ItemSource {
    pub items: HashMap<u16, ItemInfo>,
}

/// 03system `item`: the consumables (type 0 and 1) that name a command.
pub fn load(system: &[u8]) -> Result<ItemSource, Box<dyn std::error::Error>> {
    let table = data::item_table::read(bar_entry(system, b"item")?)?;
    let items = table
        .iter()
        .filter(|r| r.kind <= 1 && r.command != 0)
        .map(|r| {
            (
                r.id,
                ItemInfo {
                    amount: r.amount,
                    command: r.command,
                },
            )
        })
        .collect();
    Ok(ItemSource { items })
}

pub struct ItemRt {
    pub src: ItemSource,
    /// Sora's item slots (character record +0x34) and their auto-reload
    /// items (+0x44).
    pub slots: Vec<u16>,
    pub originals: Vec<u16>,
    /// The stock counts the camp shows and auto-reload draws on.
    pub stock: BTreeMap<u16, u8>,
    /// The slot being used and the heals waiting out their delay.
    using: Option<usize>,
    pending: Vec<(f32, u16)>,
    /// The open Items submenu's rows: the slot each row stands for.
    pub menu_slots: Vec<usize>,
}

impl ItemRt {
    pub fn new(
        src: ItemSource,
        slots: Vec<u16>,
        originals: Vec<u16>,
        stock: BTreeMap<u16, u8>,
    ) -> Self {
        ItemRt {
            src,
            slots,
            originals,
            stock,
            using: None,
            pending: Vec::new(),
            menu_slots: Vec::new(),
        }
    }

    /// The command id of the item in a slot.
    pub fn command_of(&self, slot: usize) -> Option<u16> {
        let id = *self.slots.get(slot)?;
        self.src.items.get(&id).map(|i| i.command)
    }

    /// The slot holding the item a command uses (shortcuts).
    pub fn slot_for_command(&self, command: u16) -> Option<usize> {
        (0..self.slots.len()).find(|&j| self.command_of(j) == Some(command))
    }

    /// The Items submenu rows: every filled slot, with the menu colour
    /// (5 in the air, FUN_1403b6cf0).
    pub fn menu_list(&self, airborne: bool) -> Vec<(usize, usize)> {
        (0..self.slots.len())
            .filter(|&j| self.slots[j] != 0)
            .map(|j| (j, if airborne { 5 } else { 0 }))
            .collect()
    }

    /// FUN_1403a1610: a used slot refills from the stock when the battle
    /// state drops.
    pub fn reload(&mut self) {
        for j in 0..self.slots.len() {
            let auto = self.originals.get(j).copied().unwrap_or(0);
            if self.slots[j] == 0
                && auto != 0
                && let Some(n) = self.stock.get_mut(&auto)
                && *n > 0
            {
                *n -= 1;
                self.slots[j] = auto;
            }
        }
    }
}

/// The use command for a slot (FUN_1403d6fc0 execute 3): plays motion 9
/// when Sora can take the command.
pub fn request(game: &mut Game, slot: usize, tick: u64) {
    let Game {
        player,
        clips,
        items,
        action_started,
        ..
    } = game;
    let Some(rt) = items.as_mut() else { return };
    if rt.slots.get(slot).copied().unwrap_or(0) == 0 {
        return;
    }
    // A refused state gate is silent: the "cannot" sound (se000 #5) plays
    // only on deciding a dimmed row, which the command menu already does.
    if player.use_item(clips).is_some() {
        rt.using = Some(slot);
        *action_started = tick;
    }
}

/// One tick: the use trigger consumes the slot and queues the heal; due
/// heals land.
pub fn tick(game: &mut Game, tick: u64) {
    let Game {
        player,
        items,
        enemy,
        log,
        ..
    } = game;
    let Some(rt) = items.as_mut() else { return };
    if player.take_item()
        && let Some(slot) = rt.using.take()
        && let Some(id) = rt.slots.get_mut(slot).filter(|s| **s != 0)
    {
        // FUN_1403f6790: the slot empties at the trigger.
        rt.pending.push((HEAL_DELAY, *id));
        *id = 0;
    }
    // FUN_1403f6960: the heal waits out its delay whatever Sora does.
    let mut due = Vec::new();
    rt.pending.retain_mut(|(t, id)| {
        *t -= 1.;
        if *t <= 0. {
            due.push(*id);
            false
        } else {
            true
        }
    });
    for id in due {
        let Some(info) = rt.src.items.get(&id).copied() else {
            continue;
        };
        let Some(e) = enemy.as_mut() else { continue };
        // FUN_1403c4b80: percent of the maxima, Item Boost adding to 100.
        let pct = 100 + player.abilities.item_boost as i32;
        let amount = info.amount as i32;
        let share = |max: i32| (max * amount * pct + 9999) / 10000;
        let hp = share(e.sora_target.max_hp);
        let mp = share(player.mp.max);
        let (heal_hp, heal_mp) = match id {
            1 | 2 | 5 => (hp, 0),
            3 | 6 => (0, mp),
            4 | 7 => (hp, mp),
            0x83 => (e.sora_target.max_hp, mp),
            _ => (0, 0),
        };
        if heal_hp > 0 {
            e.sora_hp = (e.sora_hp + heal_hp).min(e.sora_target.max_hp);
        }
        if heal_mp > 0 {
            player.mp.add(heal_mp);
        }
        let record = serde_json::json!({"event":"item_heal","tick":tick,"item":id,
            "hp":heal_hp,"mp":heal_mp,"sora_hp":e.sora_hp,"mp_now":player.mp.cur});
        if let Some(file) = log.as_mut() {
            let _ = serde_json::to_writer(&mut *file, &record);
            let _ = std::io::Write::write_all(file, b"\n");
        }
    }
    // The battle state drops once the enemy cannot be hit.
    if enemy.as_ref().is_some_and(|e| !e.enemy.targetable()) {
        rt.reload();
    }
}
