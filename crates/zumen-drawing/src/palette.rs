//! 色の表。系統の符号から RGB を引く。呼び出し側の表か、明示的に選んだ既定の表を使う。

mod cad;

use std::collections::BTreeMap;

use crate::ColorSystem;

/// 色の表（決定 0172）。系統と符号から RGB を引く
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Palette {
    entries: BTreeMap<(u8, u32), [u8; 3]>,
    cad: bool,
}

impl Palette {
    /// 既定の CAD 色表 v1。ACI・SXF・JWW の対応がある符号だけを解く。
    /// 利用者の表は [`Self::insert`] で上書きできる。図面内の RGB の優先は呼び出し側が保つ。
    /// 静的な表を参照するので、この操作では確保しない。
    pub fn cad() -> Self {
        Self {
            entries: BTreeMap::new(),
            cad: true,
        }
    }

    /// 系統の符号に RGB を足す。同じ符号があれば、先に足したものを使う
    pub fn insert(&mut self, system: ColorSystem, code: u32, rgb: [u8; 3]) {
        self.entries.entry((rank(system), code)).or_insert(rgb);
    }

    /// 系統の符号の RGB。表になければ `None`
    pub fn get(&self, system: ColorSystem, code: u32) -> Option<[u8; 3]> {
        self.entries
            .get(&(rank(system), code))
            .copied()
            .or_else(|| self.cad.then(|| cad::get(system, code)).flatten())
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

    #[test]
    fn cad_is_explicit_and_does_not_resolve_inheritance_or_custom_codes() {
        let palette = Palette::cad();
        for (system, code, rgb) in [
            (ColorSystem::Aci, 1, [255, 0, 0]),
            (ColorSystem::Aci, 7, [255; 3]),
            (ColorSystem::Aci, 255, [255; 3]),
            (ColorSystem::Sxf, 1, [0; 3]),
            (ColorSystem::Sxf, 16, [128; 3]),
            (ColorSystem::Jww, 1, [0, 192, 192]),
            (ColorSystem::Jww, 9, [192; 3]),
            (ColorSystem::Jww, 100, [0; 3]),
            (ColorSystem::Jww, 116, [128; 3]),
        ] {
            assert_eq!(palette.get(system, code), Some(rgb));
            assert_eq!(Palette::default().get(system, code), None);
        }
        for (system, codes) in [
            (ColorSystem::Aci, &[0, 256, u32::MAX][..]),
            (ColorSystem::Sxf, &[0, 17, u32::MAX][..]),
            (ColorSystem::Jww, &[0, 10, 99, 117, 356, u32::MAX][..]),
        ] {
            for &code in codes {
                assert_eq!(palette.get(system, code), None);
            }
        }
        for code in 1..=16 {
            assert_eq!(
                palette.get(ColorSystem::Sxf, code),
                palette.get(ColorSystem::Jww, code + 100)
            );
        }
    }

    #[test]
    fn custom_entries_override_the_preset_without_changing_it() {
        let mut palette = Palette::cad();
        palette.insert(ColorSystem::Aci, 1, [1, 2, 3]);
        palette.insert(ColorSystem::Aci, 1, [4, 5, 6]);
        palette.insert(ColorSystem::Jww, 200, [7, 8, 9]);
        assert_eq!(palette.get(ColorSystem::Aci, 1), Some([1, 2, 3]));
        assert_eq!(palette.get(ColorSystem::Jww, 200), Some([7, 8, 9]));
        assert_eq!(palette.get(ColorSystem::Aci, 2), Some([255, 255, 0]));
        assert_eq!(Palette::cad().get(ColorSystem::Aci, 1), Some([255, 0, 0]));
    }
}
