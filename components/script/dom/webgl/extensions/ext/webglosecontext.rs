/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! <https://registry.khronos.org/webgl/extensions/WEBGL_lose_context/>

use canvas_traits::webgl::{TexFormat, WebGLVersion};
use dom_struct::dom_struct;

use super::{WebGLExtension, WebGLExtensionSpec, WebGLExtensions};
use crate::dom::bindings::reflector::{DomGlobal, Reflector, reflect_dom_object};
use crate::dom::bindings::root::DomRoot;
use crate::dom::webgl::webglrenderingcontext::WebGLRenderingContext;
use crate::dom::webgl::webgltexture::{TexCompression, TexCompressionValidation};
use crate::script_runtime::CanGc;

#[dom_struct]
pub(crate) struct WEBGLLoseContext {
    reflector_: Reflector,
}

impl WEBGLLoseContext {
    fn new_inherited() -> WEBGLCompressedTextureS3TC {
        Self {
            reflector_: Reflector::new(),
        }
    }
}

impl WebGLExtension for WEBGLLoseContext {
    type Extension = WEBGLLoseContext;

    fn new(ctx: &WebGLRenderingContext, can_gc: CanGc) -> DomRoot<WEBGLCompressedTextureS3TC> {
        reflect_dom_object(
            Box::new(WEBGLLoseContext::new_inherited()),
            &*ctx.global(),
            can_gc,
        )
    }

    fn spec() -> WebGLExtensionSpec {
        WebGLExtensionSpec::Specific(WebGLVersion::WebGL1)
    }

    fn is_supported(ext: &WebGLExtensions) -> bool {
        true
    }

    fn enable(ext: &WebGLExtensions) {}

    fn name() -> &'static str {
        "WEBGL_lose_context"
    }
}
