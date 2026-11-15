//! 保存。DXF の全体保存は `zumen-acad` の [`zumen_acad::dxf::live_io::save_dxf_overlay`] を呼ぶ。

use zumen_acad::dxf::live_io::save_dxf_overlay;

use crate::store::Document;

impl Document {
    /// 文書の全体を DXF または DWG のバイト列にする。手間は、ファイルの大きさと、変えた・足した・除いたレコードの数による
    pub fn save(&self) -> Vec<u8> {
        if self.dwg.is_some() {
            return self.save_dwg();
        }
        save_dxf_overlay(
            &self.src,
            self.source_records(),
            |record| self.record_range(record),
            &self.overlay,
            &self.gone,
            self.entities_end,
        )
    }
}
