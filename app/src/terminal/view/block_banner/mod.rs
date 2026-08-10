//! "Block banners" are banners that render _inside_ a block, or its snackbar header. Currently it
//! will only render inside the active block, though that constraint can be relaxed with a bit more
//! work. The most important constraint that makes these different from other UI components is that
//! they must conform to a fixed height. This is due to an assumption we made about blocks in order
//! to efficiently viewport them: that the block height can be calculated based on Block state alone
//! without a LayoutContext. Use the exported BLOCK_BANNER_HEIGHT const when the banner height
//! needs to be taken into account.

const CONSTRAINED_BANNER_HEIGHT: f32 = 48.;
const BANNER_TOP_MARGIN: f32 = 16.;
pub const BLOCK_BANNER_HEIGHT: f32 = CONSTRAINED_BANNER_HEIGHT + BANNER_TOP_MARGIN;

pub enum WithinBlockBanner {}

impl WithinBlockBanner {
    pub fn banner_height(&self) -> f32 {
        match *self {}
    }
}
