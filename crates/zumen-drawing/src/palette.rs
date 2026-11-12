//! 色の表。系統の符号から RGB を引く。表の中身はプロファイルか呼び出し側が渡し、ここには置かない。

use std::collections::BTreeMap;

use crate::ColorSystem;

/// 色の表（決定 0172）。系統と符号から RGB を引く
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Palette {
    entries: BTreeMap<(u8, u32), [u8; 3]>,
}

impl Palette {
    /// 系統の符号に RGB を足す。同じ符号があれば、先に足したものを使う
    pub fn insert(&mut self, system: ColorSystem, code: u32, rgb: [u8; 3]) {
        self.entries.entry((rank(system), code)).or_insert(rgb);
    }

    /// 系統の符号の RGB。表になければ `None`
    pub fn get(&self, system: ColorSystem, code: u32) -> Option<[u8; 3]> {
        self.entries.get(&(rank(system), code)).copied()
    }
}

/// 表のキーに使う、系統の並び。この表の中だけで使い、外に出さない。シーンのバッファーに書く系統の符号
/// （`zumen-scene` の `layout::color_system`）とは別に持つ
fn rank(system: ColorSystem) -> u8 {
    match system {
        ColorSystem::Aci => 1,
        ColorSystem::Sxf => 2,
        ColorSystem::Jww => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_entry_wins_and_systems_do_not_mix() {
        let mut palette = Palette::default();
        palette.insert(ColorSystem::Aci, 1, [255, 0, 0]);
        palette.insert(ColorSystem::Aci, 1, [0, 0, 0]);
        palette.insert(ColorSystem::Sxf, 1, [0, 0, 255]);
        assert_eq!(palette.get(ColorSystem::Aci, 1), Some([255, 0, 0]));
        assert_eq!(palette.get(ColorSystem::Sxf, 1), Some([0, 0, 255]));
        assert_eq!(palette.get(ColorSystem::Jww, 1), None);
    }
}
