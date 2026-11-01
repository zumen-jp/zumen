//! 外部のプラグインの形式を、窓口に登録する（決定 0017・0243）。
//!
//! プラグインは、図面の形式のコーデックと、図面モデル（L2）への lift を持つ。登録は、プロセスで 1 度だけ
//! [`install`] で行い、その後に作るセッションが、コアの形式の後にプラグインの形式を試す。束ね方は 3 つある。
//!
//! - Rust: プラグインの crate を依存に足し、[`install`] してから [`crate::Session`] を作る。
//! - wasm: プラグインの crate を、コアと一緒に別のモジュールにする。モジュールは `zm_plugin_init` を出し、その
//!   中で [`install`] する。JavaScript の包みは、読み込んだときに `zm_plugin_init` を呼ぶ。
//! - ネイティブで呼ぶ言語: プラグインの crate が、[`install`] してから `zumen_cli::run` を呼ぶ CLI を作る。各言語
//!   からは、その CLI を `process` の形（決定 0013）で使う。
//!
//! プラグインの文書は読むだけで、apply で書き戻せない。

use std::sync::OnceLock;

use zumen_base::Cx;
use zumen_drawing::Drawing2D;
use zumen_sdk::{Codec, Error, Lift, Registration};

/// 図面の形式のプラグイン
#[derive(Clone, Copy, Debug)]
pub struct Plugin {
    /// 形式の登録。キーは、コアの形式とぶつからない、プラグインの台帳のキー
    pub registration: Registration,
    /// 読んで、図面モデルにする
    pub read: fn(&[u8], &mut Cx<'_>) -> Result<Drawing2D, Error>,
}

impl Plugin {
    /// コーデック `C` のプラグイン。読むときは、読んでから図面モデルに lift する
    pub const fn of<C>(registration: Registration) -> Self
    where
        C: Codec,
        C::File: Lift<Drawing2D>,
    {
        Plugin {
            registration,
            read: read::<C>,
        }
    }
}

fn read<C>(bytes: &[u8], cx: &mut Cx<'_>) -> Result<Drawing2D, Error>
where
    C: Codec,
    C::File: Lift<Drawing2D>,
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
