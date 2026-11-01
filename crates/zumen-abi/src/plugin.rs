//! 外部のプラグインの形式を、窓口に登録する（決定 0017・0243）。
//!
//! プラグインは、形式のコーデックと、図面モデルか建物モデル（L2）への lift を持つ。登録は、プロセスで 1 度だけ
//! [`install`] で行い、その後に作るセッションが、コアの形式の後にプラグインの形式を試す。束ね方は 3 つある。
//!
//! - Rust: プラグインの crate を依存に足し、[`install`] してから [`crate::Session`] を作る。
//! - wasm: プラグインの crate を、コアと一緒に別のモジュールにする。モジュールは `zm_plugin_init` を出し、その
//!   中で [`install`] する。JavaScript の包みは、読み込んだときに `zm_plugin_init` を呼ぶ。
//! - ネイティブで呼ぶ言語: プラグインの crate が、[`install`] してから `zumen_cli::run` を呼ぶ CLI を作る。各言語
//!   からは、その CLI を `process` の形（決定 0013）で使う。
//!
//! プラグインの文書は読むだけで、apply・building_apply で書き戻せない。

use std::sync::OnceLock;

use zumen_base::Cx;
#[cfg(feature = "bim")]
use zumen_building::Building;
use zumen_drawing::Drawing2D;
use zumen_sdk::{Codec, Error, Lift, Registration};

/// 形式のプラグイン
#[derive(Clone, Copy, Debug)]
pub struct Plugin {
    /// 形式の登録。キーは、コアの形式とぶつからない、プラグインの台帳のキー
    pub registration: Registration,
    /// 読んで、L2 のモデルにする
    pub read: Reader,
}

/// 読んで作る L2 のモデル
#[derive(Clone, Copy, Debug)]
pub enum Reader {
    /// 図面の形式。図面モデルにする
    Drawing(fn(&[u8], &mut Cx<'_>) -> Result<Drawing2D, Error>),
    /// 建物の形式。建物モデルにする。`bim` のモジュールで開く
    #[cfg(feature = "bim")]
    Building(fn(&[u8], &mut Cx<'_>) -> Result<Building, Error>),
}

impl Plugin {
    /// 図面の形式のコーデック `C` のプラグイン。読むときは、読んでから図面モデルに lift する
    pub const fn of<C>(registration: Registration) -> Self
    where
        C: Codec,
        C::File: Lift<Drawing2D>,
    {
        Plugin {
            registration,
            read: Reader::Drawing(read::<C, Drawing2D>),
        }
    }

    /// 建物の形式のコーデック `C` のプラグイン。読むときは、読んでから建物モデルに lift する
    #[cfg(feature = "bim")]
    pub const fn building<C>(registration: Registration) -> Self
    where
        C: Codec,
        C::File: Lift<Building>,
    {
        Plugin {
            registration,
            read: Reader::Building(read::<C, Building>),
        }
    }
}

fn read<C, M>(bytes: &[u8], cx: &mut Cx<'_>) -> Result<M, Error>
where
    C: Codec,
    C::File: Lift<M>,
{
    C::read(bytes, cx)?.lift(cx)
}

static INSTALLED: OnceLock<&'static [Plugin]> = OnceLock::new();

/// プラグインを登録する。プロセスで 1 度だけ登録でき、2 度目からは何もせずに `false` を返す。
///
/// 登録より前に開いた文書は、プラグインの形式として読み直さない
pub fn install(plugins: &'static [Plugin]) -> bool {
    INSTALLED.set(plugins).is_ok()
}

/// 登録したプラグイン
pub(crate) fn installed() -> &'static [Plugin] {
    INSTALLED.get().copied().unwrap_or(&[])
}
