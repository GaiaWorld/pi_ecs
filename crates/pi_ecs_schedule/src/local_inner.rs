//! Local 借用配对芯。
#[cfg(test)]
mod tests {
    use crate::local::Local;

    #[test]
    fn mutable_access_updates_the_retained_local_value() {
        let mut stored = String::from("before");
        {
            let mut local = Local { value: &mut stored };
            local.push_str(" after");
        }
        assert_eq!(stored, "before after");
    }

    #[test]
    fn shared_access_reads_the_borrowed_local_value() {
        let mut stored = String::from("retained");
        let local = Local { value: &mut stored };
        assert_eq!(local.as_str(), "retained");
    }
}
impl<T: Send + 'static> crate::local::Local<'_, T> {
    pub(crate) fn local_deref<'a>(_value: &'a crate::local::Local<'_, T>) -> &'a T {
        _value.value
    }
}
impl<T: Send + 'static> crate::local::Local<'_, T> {
    pub(crate) fn local_deref_mut<'a>(_value: &'a mut crate::local::Local<'_, T>) -> &'a mut T {
        _value.value
    }
}
