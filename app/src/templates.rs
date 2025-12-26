use crate::models::Link;
use askama::Template;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate;

#[derive(Template)]
#[template(path = "create_link.html")]
pub struct CreateLinkTemplate {
    pub short_link: String,
}

#[derive(Template)]
#[template(path = "links_list.html")]
pub struct LinksListTemplate {
    pub links: Vec<Link>,
    pub page: u32,
    pub has_next: bool,
    pub q: String,
}

#[derive(Template)]
#[template(path = "link_row.html")]
pub struct LinkRowTemplate {
    pub link: Link,
}

#[derive(Template)]
#[template(path = "form_error.html")]
pub struct FormErrorTemplate<'a> {
    pub message: &'a str,
}
