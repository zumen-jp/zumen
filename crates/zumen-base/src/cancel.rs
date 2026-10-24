//! 中断の合図（決定 0010）。
//!
//! 核は時計を読まない。時間の上限は、境界の側が時計を見て [`CancelToken::expire`] で伝える。
//! 合図は、予算を数えるのと同じ所で確かめる。

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::cx::Stop;

const RUNNING: u8 = 0;
const CANCELLED: u8 = 1;
const EXPIRED: u8 = 2;

/// 中断の合図。複製して、別のスレッドに渡せる。最初に届いた合図だけが効く。
#[derive(Clone, Debug, Default)]
pub struct CancelToken {
    state: Arc<AtomicU8>,
}

impl CancelToken {
    /// 合図のないものを作る
    pub fn new() -> Self {
        Self::default()
    }

    /// 中断を求める
    pub fn cancel(&self) {
        self.signal(CANCELLED);
    }

    /// 時間の上限に達したことを伝える
    pub fn expire(&self) {
        self.signal(EXPIRED);
    }

    /// 合図が届いていれば、止める理由を返す
    pub fn stop(&self) -> Option<Stop> {
        match self.state.load(Ordering::Relaxed) {
            CANCELLED => Some(Stop::Cancelled),
            EXPIRED => Some(Stop::TimedOut),
            _ => None,
        }
    }

    fn signal(&self, state: u8) {
        // 先に届いた合図を残す。失敗は、すでに合図があることを表す
        let _ = self
            .state
            .compare_exchange(RUNNING, state, Ordering::Relaxed, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_first_signal_and_shares_it_between_clones() {
        let token = CancelToken::new();
        assert_eq!(token.stop(), None);
        let other = token.clone();
        other.cancel();
        token.expire();
        assert_eq!(token.stop(), Some(Stop::Cancelled));
    }

    #[test]
    fn expire_means_the_time_limit() {
        let token = CancelToken::new();
        token.expire();
        assert_eq!(token.stop(), Some(Stop::TimedOut));
    }
}
