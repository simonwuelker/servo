/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::hash::{Hash, Hasher};
use std::sync::Mutex;

use base::generic_channel::{self, GenericSend, GenericSender};
use malloc_size_of_derive::MallocSizeOf;
use serde::{Deserialize, Serialize};
use servo_arc::Arc;
pub use servo_url::{Host, ImmutableOrigin, MutableOrigin, OriginSnapshot};
use url::Url;
use uuid::Uuid;

use crate::blob_url_store::parse_blob_url;
use crate::{FileManagerThreadMsg, ResourceThreads};

pub type ServoUrl = servo_url::BlobStoreAgnosticServoUrl<BlobToken>;
pub type ServoMaybeBlobUrl = servo_url::BlobStoreAgnosticServoUrl<BlobToken>;

use crate::{BlobTokenRefreshRequest, BlobTokenRevocationRequest, CoreResourceMsg};

#[derive(Clone, MallocSizeOf)]
pub struct BlobResolver<'a> {
    pub origin: ImmutableOrigin,
    pub resource_threads: &'a ResourceThreads,
}

#[derive(Clone, Deserialize, MallocSizeOf, Serialize)]
pub struct BlobToken {
    pub token: Uuid,
    pub file_id: Uuid,
    pub neutered: bool,
    // We need a mutex here because BlobTokens are shared among threads, and accessing
    // `GenericSender<CoreResourceMsg>` from different threads is not safe.
    //
    // We need a Arc because the Communicator is shared among different BlobTokens.
    #[conditional_malloc_size_of]
    pub communicator: Arc<Mutex<BlobTokenCommunicator>>,
}

#[derive(Clone, Deserialize, MallocSizeOf, Serialize)]
pub struct BlobTokenCommunicator {
    pub revoke_sender: GenericSender<CoreResourceMsg>,
    pub refresh_token_sender: GenericSender<CoreResourceMsg>,
}

impl servo_url::BlobToken for BlobToken {
    fn refresh(&self) -> Self {
        let (new_token_sender, new_token_receiver) = generic_channel::channel().unwrap();
        let refresh_request = BlobTokenRefreshRequest {
            blob_id: self.file_id.clone(),
            new_token_sender,
        };
        self.communicator
            .lock()
            .unwrap()
            .refresh_token_sender
            .send(CoreResourceMsg::RefreshTokenForFile(refresh_request))
            .unwrap();
        let new_token = new_token_receiver.recv().unwrap();

        BlobToken {
            token: new_token,
            file_id: self.file_id.clone(),
            communicator: self.communicator.clone(),
            neutered: false,
        }
    }

    fn neuter(&mut self) {
        self.neutered = true;
    }
}

impl<'a> BlobResolver<'a> {
    pub fn acquire_blob_token_for(
        &self,
        url: &Url,
    ) -> Option<servo_url::TokenSerializationGuard<BlobToken>> {
        if url.scheme() != "blob" {
            return None;
        }
        let (file_id, origin) = parse_blob_url(url)
            .inspect_err(|error| log::warn!("Failed to acquire token for {url}: {error}"))
            .ok()?;
        let (sender, receiver) = generic_channel::channel().unwrap();
        self.resource_threads
            .send(CoreResourceMsg::ToFileManager(
                FileManagerThreadMsg::GetTokenForFile(file_id, origin, sender),
            ))
            .ok()?;
        let reply = receiver.recv().ok()?;
        let serializable_token = reply.token.map(|token_id| {
            servo_url::TokenSerializationGuard::new(BlobToken {
                token: token_id,
                file_id,
                communicator: Arc::new(Mutex::new(BlobTokenCommunicator {
                    revoke_sender: reply.revoke_sender,
                    refresh_token_sender: reply.refresh_sender,
                })),
                neutered: false,
            })
        });
        serializable_token
    }
}

impl Drop for BlobToken {
    fn drop(&mut self) {
        if self.neutered {
            return;
        }

        let revocation_request = BlobTokenRevocationRequest {
            token: self.token.clone(),
            blob_id: self.file_id.clone(),
        };
        let _ = self
            .communicator
            .lock()
            .unwrap()
            .revoke_sender
            .send(CoreResourceMsg::RevokeTokenForFile(revocation_request));
    }
}

impl Eq for BlobToken {}

impl Hash for BlobToken {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.token.hash(state);
    }
}
impl Ord for BlobToken {
    fn cmp(&self, other: &BlobToken) -> std::cmp::Ordering {
        self.token.cmp(&other.token)
    }
}
impl PartialOrd for BlobToken {
    fn partial_cmp(&self, other: &BlobToken) -> Option<std::cmp::Ordering> {
        self.token.partial_cmp(&other.token)
    }
}
impl PartialEq for BlobToken {
    fn eq(&self, other: &BlobToken) -> bool {
        self.token == other.token
    }
}
