//! 变化时间配对芯；只做非零值封装，不读取 World。
use super::ChangeTick;
impl ChangeTick {
    pub(super) fn new_inner(value: std::num::NonZeroU64) -> Self {
        Self(value)
    }
    pub(super) fn get_inner(self) -> u64 {
        self.0.get()
    }
}
