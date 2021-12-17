pub trait Resource: 'static + Send + Sync {}

impl<T: 'static + Send + Sync> Resource for T {}