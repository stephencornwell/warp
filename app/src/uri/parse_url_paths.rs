use url::Url;

#[derive(PartialEq, Debug)]
pub enum WarpWebLink {
    Session,
}

pub fn get_item_data_from_warp_link(url: &Url) -> Option<WarpWebLink> {
    let _ = url;
    None
}
