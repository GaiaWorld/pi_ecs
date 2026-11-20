//! 内置数组特殊释放；类型化安装已归 BundleWriter，无擦除初始化描述。
#[path = "builtin_init_inner.rs"]
mod builtin_init_inner;
/// # 用途
/// 特殊释放具体 StrongEntities<N,T>，逐位置交还 strong；分类与初始化使用同一单态类型。
/// # Safety
/// context 属于原 World 且有释放授权；value 为该具体类型唯一活值责任，不得再普通析构。
pub(crate) unsafe fn release_strong<const N: usize, T>(
    context: &crate::destruction::ReleaseContext<'_>,
    value: crate::ptr::OwningPtr<'_>,
) {
    unsafe { builtin_init_inner::release_strong_inner::<N, T>(context, value) }
}
/// # 用途
/// 特殊释放具体 WeakEntities<N,T>，逐位置交还 weak，归零只登记。
/// # Safety
/// 同 release_strong，value 的具体类型必须是 WeakEntities<N,T>。
pub(crate) unsafe fn release_weak<const N: usize, T>(
    context: &crate::destruction::ReleaseContext<'_>,
    value: crate::ptr::OwningPtr<'_>,
) {
    unsafe { builtin_init_inner::release_weak_inner::<N, T>(context, value) }
}
