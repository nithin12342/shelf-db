use crate::storage::StorageBackend;

pub fn storage() -> Box<dyn StorageBackend> {
    #[cfg(sheaf_native_storage)]
    {
        Box::new(crate::storage::NativeStorage)
    }
    #[cfg(not(sheaf_native_storage))]
    {
        Box::new(crate::storage::FallbackStorage)
    }
}
