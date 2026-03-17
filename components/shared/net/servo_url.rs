/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::hash::{Hash, Hasher};
use std::sync::Mutex;

use base::generic_channel::{self, GenericSender};
use malloc_size_of_derive::MallocSizeOf;
use serde::{Deserialize, Serialize};
use servo_arc::Arc;
pub use servo_url::{ImmutableOrigin, MutableOrigin};
use url::Url;
use uuid::Uuid;

use crate::FileManagerThreadMsg;
use crate::blob_url_store::parse_blob_url;

pub type ServoUrl = servo_url::BlobStoreAgnosticServoUrl<BlobResolver>;

use crate::{BlobTokenRefreshRequest, BlobTokenRevocationRequest, CoreResourceMsg};

#[derive(Clone, MallocSizeOf)]
pub struct BlobResolver {
    /// Channels that are given to BlobTokens for communicating with the
    /// file manager thread.
    #[conditional_malloc_size_of]
    communicator: Arc<Mutex<BlobTokenCommunicator>>,
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

impl servo_url::BlobStorage for BlobResolver {
    type Token = BlobToken;

    fn acquire_blob_token(
        &self,
        url: &Url,
    ) -> Result<Option<servo_url::TokenSerializationGuard<Self::Token>>, ()> {
        if url.scheme() != "blob" {
            return Ok(None);
        }
        let Ok((file_id, origin)) = parse_blob_url(url) else {
            return Ok(None);
        };
        let (sender, receiver) = generic_channel::channel().unwrap();
        self.communicator
            .lock()
            .unwrap()
            .refresh_token_sender
            .send(CoreResourceMsg::ToFileManager(
                FileManagerThreadMsg::GetTokenForFile(file_id, origin, sender),
            ))
            .map_err(|_| ())?;
        let Ok(reply) = receiver.recv() else {
            return Err(());
        };
        let serializable_token = reply.token.map(|token_id| {
            servo_url::TokenSerializationGuard::new(BlobToken {
                token: token_id,
                file_id,
                communicator: self.communicator.clone(),
                neutered: false,
            })
        });
        Ok(serializable_token)
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
