//! 「コアとプラグイン」の CLI（決定 0017・0243）。プラグインを登録してから、コアの CLI を動かす。

use std::process::ExitCode;

fn main() -> ExitCode {
    zumen_abi1::install(zumen_plugin_template::PLUGINS);
    zumen_cli::run()
}
