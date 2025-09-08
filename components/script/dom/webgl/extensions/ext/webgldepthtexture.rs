/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;

use crate::dom::bindings::reflector::{DomGlobal, Reflector, reflect_dom_object};
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::webgl::webglrenderingcontext::WebGLRenderingContext;
use crate::script_runtime::CanGc;
use crate::dom::extensions::WebGLExtensions;
use crate::dom::extensions::WebGLExtensionSpec;
use crate::dom::types::ANGLEInstancedArrays;
use crate::dom::extensions::WebGLExtension;
use canvas_traits::webgl::WebGLVersion;

/// <https://registry.khronos.org/webgl/extensions/WEBGL_depth_texture/>
#[dom_struct]
pub(crate) struct WEBGLDepthTexture {
    reflector_: Reflector,
    ctx: Dom<WebGLRenderingContext>,
}

impl WEBGLDepthTexture {
    fn new_inherited(ctx: &WebGLRenderingContext) -> Self {
        Self {
            reflector_: Reflector::new(),
            ctx: Dom::from_ref(ctx),
        }
    }
}

impl WebGLExtension for WEBGLDepthTexture {
    type Extension = Self;

    fn new(ctx: &WebGLRenderingContext, can_gc: CanGc) -> DomRoot<Self> {
        reflect_dom_object(
            Box::new(WEBGLDepthTexture::new_inherited(ctx)),
            &*ctx.global(),
            can_gc,
        )
    }

    fn spec() -> WebGLExtensionSpec {
        WebGLExtensionSpec::All
    }

    fn is_supported(ext: &WebGLExtensions) -> bool {
        dbg!(ext.supports_gl_extension("GL_ANGLE_depth_texture"))
    }

    fn enable(ext: &WebGLExtensions) {
        println!("enabling depth texture extensions");
    }

    fn name() -> &'static str {
        "ANGLE_depth_texture"
    }
}
