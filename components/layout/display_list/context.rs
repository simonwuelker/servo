use net_traits::image_cache::ImageCache;
use paint_api::CrossProcessPaintApi;

pub(crate) struct ClipTreeContext<'a> {
    pub paint_api: &'a CrossProcessPaintApi,
    pub image_cache: &'a dyn ImageCache,
}

impl<'a> ClipTreeContext<'a> {
    pub(crate) fn foo(&self) {
        // self.paint_api.add_image(image_key, descriptor, data, false);

        // if let Some(old_image_key) = self.image_key.replace(image_key) {
        //     self.paint_api.delete_image(old_image_key);
        // }
    }
}
