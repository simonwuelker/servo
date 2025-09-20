use serde::{Deserialize, Serialize};
use url::Origin;
use uuid::Uuid;

pub trait BlobLock {
    type Token: Serialize + Deserialize;

    fn acquire_blob_lock(origin: Origin, id: Uuid) -> Self::Token;
}
